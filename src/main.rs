use std::future::IntoFuture;

use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use evalexpr::{build_operator_tree, Node};
use futures_util::StreamExt;
use log::{debug, error, info, trace, warn};
use tower_http::cors::{Any, CorsLayer};

use indexer_rabbitmq::lapin::{
    options::{BasicQosOptions, QueueDeclareOptions},
    Channel, Queue,
};
use socketioxide::{
    adapter::Room,
    extract::{Data, SocketRef},
    SocketIo,
};
use step_ingestooor_engine::rabbit_factory;
use step_ingestooor_sdk::schema::{Schema, SchemaTrait};

#[derive(Parser, PartialEq, Debug)]
pub struct BroadcastooorArgs {
    /// The address of an AMQP server to connect to
    #[clap(long, env)]
    pub rabbitmq_url: String,

    /// The exchange to use
    #[clap(long, env)]
    pub rabbitmq_exchange: String,

    /// The rabbitMQ prefetch count to use when reading from queues
    /// This loosely translates to # simultaneous messages being processed
    #[clap(long, env)]
    pub rabbitmq_prefetch: Option<u16>,
}

#[derive(serde::Deserialize)]
pub struct Filter {
    id: String,
    expression: String,
}

#[derive(serde::Deserialize)]
pub struct SubscribeRequest {
    topic: String,
    filter: Option<Filter>,
}

#[derive(serde::Deserialize)]
pub struct UnsubscribeRequest {
    topic: String,
    filter_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Message {
    topic: String,
    filter_id: Option<String>,
    schema: Schema,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = BroadcastooorArgs::parse();

    //rabbit setup
    let connection = rabbit_factory::amqp_connect(args.rabbitmq_url, "broadcastooor").await?;
    let channel = connection.create_channel().await?;
    let queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                auto_delete: true,
                durable: false,
                exclusive: true,
                ..Default::default()
            },
            Default::default(),
        )
        .await?;
    channel
        .queue_bind(
            queue.name().as_str(),
            &args.rabbitmq_exchange,
            "#",
            Default::default(),
            Default::default(),
        )
        .await?;

    //socket server setup
    let (io_layer, io) = SocketIo::new_layer();

    //socket handlers simply subscribe and unsubscribe from topics
    io.ns("/data_schema", |s: SocketRef| {
        //create the filter map on all sockets
        let filters = DashMap::<String, DashMap<String, Option<Node>>>::new();
        s.extensions.insert(filters);
        //create the handlers
        s.on("subscribe", handle_subscribe);
        s.on("unsubscribe", handle_unsubscribe);
    });

    //create a thread that uses rabbit to listen and publish schemas
    let publisher_thread = tokio::spawn(rabbit_thread(
        channel,
        queue,
        args.rabbitmq_prefetch.unwrap_or(64_u16),
        io,
    ));

    //build the tower layers
    let cors_layer = CorsLayer::new()
        //testing - allow requests from any origin
        .allow_origin(Any);
    let app = axum::Router::new().layer(io_layer).layer(cors_layer);

    //create the sucket server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let app_thread = axum::serve(listener, app).into_future();

    info!("started listening on all local IPs; port 3000");

    //wait for either thread to fail
    tokio::select! {
        Err(e) = publisher_thread => {
            error!("publisher thread failed {}", e);
        }
        Err(e) = app_thread => {
            error!("app thread failed {}", e);
        }
    };

    error!("broadcastooor exiting");
    Ok(())
}

async fn rabbit_thread(channel: Channel, queue: Queue, prefetch: u16, io: SocketIo) {
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
                        if let Some(room_filters) = all_filters.get(topic) {
                            for room_filter in room_filters.iter() {
                                if let Some(filter) = room_filter.value() {
                                    debug!("found room filter {}", filter.to_string());
                                    match filter.eval_boolean_with_context(&context) {
                                        Ok(true) => {
                                            let message = Message {
                                                topic: topic.clone(),
                                                filter_id: Some(room_filter.key().clone()),
                                                schema: schema.clone(),
                                            };

                                            //debug
                                            let message = serde_json::to_string(&message)
                                                .expect("failed to serialize message");

                                            socket.emit("message", message).ok();
                                        }
                                        Ok(false) => {
                                            //do nothing
                                            debug!(
                                                "filter {} evaluated to false",
                                                room_filter.key()
                                            );
                                        }
                                        Err(e) => {
                                            error!("filter evaluation failed: {}", e);
                                            socket
                                                .emit(
                                                    "error",
                                                    format!("filter evaluation failed: {}", e),
                                                )
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
                                    let message = serde_json::to_string(&message)
                                        .expect("failed to serialize message");

                                    socket.emit("message", message).ok();
                                }
                            }
                        }
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

fn handle_subscribe(s: SocketRef, msg: Data<String>) {
    let Data(msg) = msg;
    let msg = match serde_json::from_str::<SubscribeRequest>(&msg) {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };

    info!("received subscribe for {}", msg.topic);

    //get a reference to filters on the socket
    let all_filters = s
        .extensions
        .get_mut::<DashMap<String, DashMap<String, Option<Node>>>>()
        .unwrap();
    //get the room filters or create a new one
    let room_filters = all_filters
        .entry(msg.topic.clone())
        .or_insert_with(DashMap::<String, Option<Node>>::new);

    //add filter for the room
    if let Some(filter) = msg.filter {
        match build_operator_tree(&filter.expression) {
            Ok(tree) => {
                //insert the filter into the room filters
                room_filters.value().insert(filter.id.clone(), Some(tree));
            }
            Err(e) => {
                error!("failed to parse filter expression: {}", e);
                s.emit("error", format!("subscribe error: {}", e)).ok();
                return;
            }
        }
        debug!(
            "subscribed to {} with filter {} expr {}",
            msg.topic, filter.id, filter.expression
        );
    } else {
        //insert an empty placeholder to represent the room subscription, this is to make sure we don't unsubscribe from the room
        //when we unsubscribe from the last filter if we also subscribed to the room
        room_filters.value().insert(String::from(""), None);
        debug!("subscribed to {}", msg.topic);
    }
    //join the room for socketio
    s.join(Room::Owned(msg.topic.clone())).ok();

    //notify the client that they have subscribed
    s.emit("subscribed", msg.topic).ok();
}

fn handle_unsubscribe(s: SocketRef, msg: Data<String>) {
    let Data(msg) = msg;
    let msg = match serde_json::from_str::<UnsubscribeRequest>(&msg) {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse unsubscribe request: {}", e);
            s.emit("error", format!("unsubscribe error: {}", e)).ok();
            return;
        }
    };

    //get a reference to filters on the socket
    let all_filters = s
        .extensions
        .get_mut::<DashMap<String, DashMap<String, Option<Node>>>>()
        .unwrap();

    //grab the room filters
    if let Some(room_filters) = all_filters.get_mut(&msg.topic) {
        //leave the filter for the room
        if let Some(filter_id) = msg.filter_id {
            if room_filters.remove(&filter_id).is_none() {
                debug!(
                    "no filter found for {} with filter {}",
                    msg.topic, filter_id
                );
                return;
            }
            debug!("unsubscribed from {} with filter {}", msg.topic, filter_id);
        } else {
            room_filters.remove("");
            debug!("unsubscribed from {}", msg.topic);
        }
        if room_filters.is_empty() {
            //if there are no more filters for the room, leave the room and delete the room filters entry
            s.leave(msg.topic.clone()).ok();
            all_filters.remove(&msg.topic);
        }
    } else {
        warn!("somehow no room filters for {}", msg.topic);
        //register a leave, but not sure how in this state
        s.leave(msg.topic.clone()).ok();
    }
    //notify the client that they have unsubscribed
    s.emit("unsubscribed", msg.topic).ok();
}
