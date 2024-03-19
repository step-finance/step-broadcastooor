use std::future::IntoFuture;

use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use log::{debug, error, trace};
use tower_http::cors::{Any, CorsLayer};
use http::Method;

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
            &queue.name().as_str(),
            &args.rabbitmq_exchange,
            "#",
            Default::default(),
            Default::default(),
        )
        .await?;

    //socket server setup
    let (io_layer, io) = SocketIo::new_layer();

    io.ns("/data_schema", |s: SocketRef| {
        s.on("subscribe", |s: SocketRef, Data::<String>(msg)| {
            let join_result = s.join(Room::Owned(msg.clone()));
            if join_result.is_err() {
                error!("failed to join room {}", msg);
                return;
            }
            debug!("subscribed to {}", msg);
            s.emit("subscribed", msg).ok();
        });
        s.on("unsubscribe", |s: SocketRef, Data::<String>(msg)| {
            let leave_result = s.leave(Room::Owned(msg.clone()));
            if leave_result.is_err() {
                error!("failed to leave room {}", msg);
                return;
            }
            debug!("unsubscribed from {}", msg);
            s.emit("unsubscribed", msg).ok();
        });
    });

    let publisher_thread = tokio::spawn(rabbit_thread(
        channel,
        queue,
        args.rabbitmq_prefetch.unwrap_or(64 as u16),
        io,
    ));

    let cors_layer = CorsLayer::new()
        // allow requests from any origin
        .allow_origin(Any);
    let app = axum::Router::new().layer(io_layer).layer(cors_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let app_thread = axum::serve(listener, app).into_future();
    
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
    channel
        .basic_qos(prefetch, BasicQosOptions::default())
        .await
        .expect("failed to set qos");

    let consumer = channel
        .basic_consume(
            queue.name().as_str(),
            "broadcastooor",
            Default::default(),
            Default::default(),
        )
        .await
        .expect("failed to consume");

    consumer
        .map(|delivery| (delivery, io.clone()))
        .for_each_concurrent(None, move |t| async move {
            let delivery = t.0.expect("failed to get delivery");
            let data = delivery.data.as_slice();

            trace!("got message: {:?}", String::from_utf8(data.to_vec()).unwrap());

            let schemas: Vec<Schema> =
                serde_json::from_slice(data).expect("failed to parse message");

            for schema in schemas {
                let topics = schema.get_topics();
                let schema_string = serde_json::to_string(&schema).unwrap();
                t.1.of("/data_schema").unwrap().to(topics).emit("data", schema_string).ok();
            }
            delivery
                .ack(Default::default())
                .await
                .expect("failed to ack");
        })
        .await;
}
