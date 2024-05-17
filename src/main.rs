use std::future::IntoFuture;

use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use evalexpr::{build_operator_tree, Node};
use log::{debug, error, info, warn};
use tower_http::cors::{Any, CorsLayer};

use indexer_rabbitmq::lapin::options::QueueDeclareOptions;
use messages::{Message, SubscribeRequest, UnsubscribeRequest};
use socketioxide::{
    adapter::Room,
    extract::{SocketRef, TryData},
    SocketIo,
};
use step_ingestooor_engine::rabbit_factory;

mod messages;
mod receiver;

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
    let publisher_thread = tokio::spawn(receiver::create_rabbit_thread(
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

fn handle_subscribe(s: SocketRef, msg: TryData<String>) {
    let TryData::<String>(msg) = msg;
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };
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

fn handle_unsubscribe(s: SocketRef, msg: TryData<String>) {
    let TryData(msg) = msg;
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };
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
