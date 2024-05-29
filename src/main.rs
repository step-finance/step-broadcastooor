//! The broadcastooor is a service that listens to a rabbitMQ exchange and broadcasts messages to clients via SocketIO.
//!
//! Requires environment variables or arguments as defined in the [BroadcastooorArgs] struct.
//!
//! The SocketIO path is `/data_schema` and supported messages are:
//! - `subscribe`: Subscribe to a topic, payload should be [SubscribeRequest]
//! - `unsubscribe`: Unsubscribe from a topic, payload should be [UnsubscribeRequest]
//!
//! Events that are emitted:
//! - `schema`: A topic subscribed to emits a schema, payload is [SchemaMessage]
//! - `subscribed`: Successfully subscribed to a topic, payload is the topic name
//! - `unsubscribed`: Successfully unsubscribed from a topic, payload is the topic name
//! - `error`: An error occurred, payload is a string describing the error
//!
//! The format for topics is `<schema_name>.<field name>.<field value>`. For instance,
//! a `SolTransfer` schema with a `source` field of `123` would have a topic of `SolTransfer.source.123`.
//! Some schemas are also published as general topics as just `<schema_name>`.
//!
//! For specific schemas, and their fields exposed as topics, see the [step_ingestooor_sdk::schema] module.
use std::future::IntoFuture;

use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use evalexpr::Node;
use log::{error, info};
use tower_http::cors::{Any, CorsLayer};

use indexer_rabbitmq::lapin::{options::QueueDeclareOptions, types::FieldTable};
use socketioxide::{extract::SocketRef, SocketIo};
use step_ingestooor_engine::rabbit_factory;

use crate::handlers::{subscribe::handle_subscribe, unsubscribe::handle_unsubscribe};

#[doc(inline)]
pub use messages::*;

#[doc(hidden)]
mod handlers;
#[doc(hidden)]
mod messages;
#[doc(hidden)]
mod receiver;

/// The path to the socket.io namespace that handles schema subscriptions
const SCHEMA_SOCKETIO_PATH: &str = "/data_schema";
/// the path for a healthcheck endpoint
const HEATHCHECK_PATH: &str = "/healthcheck";
/// The address and port to bind the socket server to
const BIND_ADDR_PORT: &str = "0.0.0.0:3000";

/// The arguments for the broadcastooor. These can be passed as arguments or environment variables.
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

#[doc(hidden)]
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = BroadcastooorArgs::parse();

    //rabbit setup
    let connection = rabbit_factory::amqp_connect(args.rabbitmq_url, "broadcastooor").await?;
    let channel = connection.create_channel().await?;
    //create the temp queue with a max backlog of 10k.
    //if we can't keep up, theres a problem, but we don't want to just pile on rabbit
    let mut arguments = FieldTable::default();
    arguments.insert("max-length".into(), 10_000.into());
    let queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                auto_delete: true,
                durable: false,
                exclusive: true,
                ..Default::default()
            },
            arguments,
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
    io.ns(SCHEMA_SOCKETIO_PATH, |s: SocketRef| {
        //create the filter map on all sockets
        let filters = DashMap::<String, DashMap<String, Option<Node>>>::new();
        s.extensions.insert(filters);
        //create the handlers
        s.on("subscribe", handle_subscribe);
        s.on("unsubscribe", handle_unsubscribe);
    });

    //create a thread that uses rabbit to listen and publish schemas
    let publisher_thread = tokio::spawn(receiver::run_rabbit_thread(
        channel,
        queue,
        args.rabbitmq_prefetch.unwrap_or(64_u16),
        io,
    ));

    //build the tower layers
    let cors_layer = CorsLayer::new()
        //testing - allow requests from any origin
        .allow_origin(Any);
    let app = axum::Router::new()
        //healthcheck for aws
        .route(HEATHCHECK_PATH, axum::routing::get(|| async { "ok" }))
        //socketio
        .layer(io_layer)
        //cors
        .layer(cors_layer);

    //create the socket server
    let listener = tokio::net::TcpListener::bind(BIND_ADDR_PORT).await.unwrap();
    let app_thread = axum::serve(listener, app).into_future();

    info!("started listening on {}", BIND_ADDR_PORT);

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
