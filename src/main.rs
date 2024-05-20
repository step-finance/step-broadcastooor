use std::future::IntoFuture;

use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use evalexpr::Node;
use log::{error, info};
use tower_http::cors::{Any, CorsLayer};

use indexer_rabbitmq::lapin::options::QueueDeclareOptions;
use socketioxide::{extract::SocketRef, SocketIo};
use step_ingestooor_engine::rabbit_factory;

use crate::handlers::{subscribe::handle_subscribe, unsubscribe::handle_unsubscribe};

mod handlers;
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
