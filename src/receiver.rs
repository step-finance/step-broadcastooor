use dashmap::DashMap;
use evalexpr::Node;
use futures_util::StreamExt;
use indexer_rabbitmq::lapin::{options::BasicQosOptions, Channel, Queue};
use log::{debug, error, trace};
use socketioxide::{extract::SocketRef, SocketIo};
use step_ingestooor_sdk::schema::{Schema, SchemaTrait};

use crate::messages::Message;

pub async fn create_rabbit_thread(channel: Channel, queue: Queue, prefetch: u16, io: SocketIo) {
    //set the prefetch on the channel
    channel
        .basic_qos(prefetch, BasicQosOptions::default())
        .await
        .expect("failed to set qos");

    //create a message consumer
    let consumer = channel
        .basic_consume(
            queue.name().as_str(),
            "broadcastooor",
            Default::default(),
            Default::default(),
        )
        .await
        .expect("failed to consume");

    //process messages async (neverending loop)
    consumer
        .map(|delivery| (delivery, io.clone()))
        .for_each_concurrent(None, move |t| async move {
            let delivery = t.0.expect("failed to get delivery");
            let data = delivery.data.as_slice();

            trace!("got message: {}", String::from_utf8(data.to_vec()).unwrap());

            let schemas: Vec<Schema> =
                serde_json::from_slice(data).expect("failed to parse message");

            for schema in schemas {
                let topics = schema.get_topics();

                trace!(
                    "publishing schema {} to topics: {:?}",
                    schema.get_schema_name(),
                    topics
                );

                //because of expr evaluation, we need to manually loop the sockets
                let sockets_for_eval =
                    t.1.of("/data_schema")
                        .unwrap()
                        .to(topics.clone())
                        .sockets()
                        .unwrap();

                let cnt = sockets_for_eval.len();
                if cnt > 0 {
                    debug!("got {} sockets for eval", cnt);
                } else {
                    //no sockets, no need to continue
                    continue;
                }

                //TODO maybe short circuit this loop with a check to see if any socket has a filter on any of the topics
                //     if not, we can just emit to all and move ondebug_exchange
                let context = schema.get_expr_context();
                for socket in sockets_for_eval {
                    //safe to unwrap, is always created in the subscribe handler
                    let all_filters = socket
                        .extensions
                        .get::<DashMap<String, DashMap<String, Option<Node>>>>()
                        .unwrap();

                    //REMOVE THIS
                    // let mut context = evalexpr::HashMapContext::new();
                    // evalexpr::ContextWithMutableVariables::set_value(&mut context, "asd".to_string(), evalexpr::Value::Int(100u64.try_into().or_else(|e|i64::MAX).unwrap()))
                    //     .or_else(|e| {
                    //         log::error!("Failed to set value for {} cause: {}", "asd", e);
                    //         Ok(())
                    //     }).ok();

                    //for each topic, look for filters and evaluate them
                    for topic in topics.iter() {
                        handle_schema(&all_filters, topic, &context, &schema, &socket);
                    }
                }
            }
            delivery
                .ack(Default::default())
                .await
                .expect("failed to ack");
        })
        .await;
}

fn handle_schema(
    all_filters: &socketioxide::extensions::Ref<DashMap<String, DashMap<String, Option<Node>>>>,
    topic: &String,
    context: &evalexpr::HashMapContext,
    schema: &Schema,
    socket: &SocketRef,
) {
    if let Some(room_filters) = all_filters.get(topic) {
        for room_filter in room_filters.iter() {
            if let Some(filter) = room_filter.value() {
                debug!("found room filter {}", filter.to_string());
                match filter.eval_boolean_with_context(context) {
                    Ok(true) => {
                        let message = Message {
                            topic: topic.clone(),
                            filter_id: Some(room_filter.key().clone()),
                            schema: schema.clone(),
                        };

                        //debug
                        let message =
                            serde_json::to_string(&message).expect("failed to serialize message");

                        socket.emit("message", message).ok();
                    }
                    Ok(false) => {
                        //do nothing
                        debug!("filter {} evaluated to false", room_filter.key());
                    }
                    Err(e) => {
                        error!("filter evaluation failed: {}", e);
                        socket
                            .emit("error", format!("filter evaluation failed: {}", e))
                            .ok();
                    }
                }
            } else {
                //this is a full room subscription, send the schema to the client
                let message = Message {
                    topic: topic.clone(),
                    filter_id: None,
                    schema: schema.clone(),
                };

                //debug
                let message = serde_json::to_string(&message).expect("failed to serialize message");

                socket.emit("message", message).ok();
            }
        }
    }
}
