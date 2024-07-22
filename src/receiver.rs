use std::sync::Arc;

use dashmap::DashMap;
use evalexpr::Node;
use futures_util::StreamExt;
use indexer_rabbitmq::lapin::{
    options::{BasicConsumeOptions, BasicQosOptions},
    Channel, Queue,
};
use log::{debug, error, trace, warn};
use socketioxide::{extract::SocketRef, socket::Socket, SocketIo};
use step_ingestooor_sdk::schema::{Schema, SchemaTrait};
use tokio::task;

use crate::{messages::SchemaMessage, TopicFilterMap, SCHEMA_SOCKETIO_PATH};

pub const RECV_SCHEMA_EVENT_NAME: &str = "receivedSchema";

pub async fn run_rabbit_thread(channel: Channel, queue: Queue, prefetch: u16, io: SocketIo) {
    //set the prefetch on the channel
    channel
        .basic_qos(prefetch, BasicQosOptions::default())
        .await
        .expect("failed to set qos");

    //create a message consumer
    let mut consumer = channel
        .basic_consume(
            queue.name().as_str(),
            "broadcastooor",
            BasicConsumeOptions {
                no_ack: true,
                exclusive: true,
                nowait: true,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .expect("failed to consume");

    //process messages async (neverending loop)
    while let Some(message_result) = consumer.next().await {
        if let Ok(delivery) = message_result {
            let io = io.clone();
            task::spawn(async move {
                handle_incoming_schemas(delivery.data, io);
            });
        } else {
            error!("failed to get message");
        }
    }
}

fn handle_incoming_schemas(data: Vec<u8>, socket_io: SocketIo) {
    trace!("got message: {}", String::from_utf8(data.to_vec()).unwrap());

    let schemas: Vec<Schema> = serde_json::from_slice(&data).expect("failed to parse message");

    for schema in schemas {
        let topics = schema.get_topics();

        trace!(
            "publishing schema {} to topics: {:?}",
            schema.get_schema_name(),
            topics
        );

        for topic in topics.iter() {
            let sockets_for_eval = socket_io
                .of(SCHEMA_SOCKETIO_PATH)
                .unwrap()
                .to(topic.clone())
                .sockets()
                .unwrap();

            handle_topic(sockets_for_eval, topic, &schema);
        }
    }
}

fn handle_topic(sockets_for_eval: Vec<SocketRef>, topic: &str, schema: &Schema) {
    let cnt = sockets_for_eval.len();
    if cnt > 0 {
        debug!("got {} sockets for eval", cnt);
    } else {
        //no sockets, no need to continue
        return;
    }

    let context = schema.get_expr_context();

    //because of expr evaluation, we need to manually loop the sockets
    for socket in sockets_for_eval {
        //safe to unwrap, is always created in the subscribe handler
        let all_filters = socket.extensions.get::<Arc<TopicFilterMap>>().unwrap();

        handle_socket(schema, topic, &socket, &all_filters, &context);
    }
}

fn handle_socket(
    schema: &Schema,
    topic: &str,
    socket: &Socket,
    all_filters: &DashMap<String, DashMap<String, Option<Node>>>,
    context: &evalexpr::HashMapContext,
) {
    let mut bad_filters: Option<Vec<String>> = None;
    let mut empty_room = false;
    if let Some(room_filters) = all_filters.get(topic) {
        for room_filter in room_filters.iter() {
            if let Some(filter) = room_filter.value().as_ref() {
                let filter_id = room_filter.key();
                if !handle_filter(schema, topic, socket, filter_id, filter, context) {
                    //this filter is erroring, remove it
                    if bad_filters.is_none() {
                        bad_filters = Some(Vec::new());
                    }
                    if let Some(bad_filters) = &mut bad_filters {
                        bad_filters.push(filter_id.clone());
                    }
                }
            } else {
                //this is a full room subscription, send the schema to the client
                let message = SchemaMessage {
                    topic: topic.to_owned(),
                    filter_id: None,
                    schema: schema.clone(),
                };

                socket.emit(RECV_SCHEMA_EVENT_NAME, message).ok();
            }
        }
        if bad_filters.is_some() {
            for bad_filter in bad_filters.unwrap() {
                let removed = room_filters.remove(&bad_filter);
                warn!(
                    "filter {} with expr {} failed and removed from subscriptions",
                    bad_filter,
                    removed
                        .unwrap_or_default()
                        .1
                        .map(|a| a.to_string())
                        .unwrap_or_default()
                );
                if let Err(e) = socket.emit(
                    "serverError",
                    format!(
                        "filter {} failed and removed from subscriptions",
                        bad_filter
                    ),
                ) {
                    error!("failed to emit serverError: {}", e);
                }
            }
            if room_filters.len() == 0 {
                //no filters left, remove the room
                empty_room = true;
            }
        }
    }
    if empty_room {
        debug!(
            "handle socket: no more filters for room, leaving room {}",
            topic
        );
        if let Err(e) = socket.leave(topic.to_owned()) {
            error!("failed to leave room: {}", e);
        }
    }
}

fn handle_filter(
    schema: &Schema,
    topic: &str,
    socket: &Socket,
    filter_id: &String,
    filter: &Node,
    context: &evalexpr::HashMapContext,
    //return true if the filter was evaluated successfully
) -> bool {
    debug!("found room filter {}", filter.to_string());
    match filter.eval_boolean_with_context(context) {
        Ok(true) => {
            let message = SchemaMessage {
                topic: topic.to_owned(),
                filter_id: Some(filter_id),
                schema: schema.clone(),
            };
            debug!("filter {} evaluated to true", filter_id);
            socket.emit(RECV_SCHEMA_EVENT_NAME, message).ok();
            true
        }
        Ok(false) => {
            //do nothing
            debug!("filter {} evaluated to false", filter_id);
            true
        }
        Err(e) => {
            error!("filter evaluation failed: {}", e);
            socket
                .emit("error", format!("filter evaluation failed: {}", e))
                .ok();
            false
        }
    }
}
