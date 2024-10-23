//! The broadcastooor is a service that listens to a rabbitMQ exchange and broadcasts messages to clients via SocketIO.
//!
//! Requires environment variables or arguments as defined in the [BroadcastooorArgs] struct.
//!
//! The SocketIO path is `/dooots` and supported messages are:
//! - `subscribe`: Subscribe to a topic, payload should be [SubscribeRequest]
//! - `unsubscribe`: Unsubscribe from a topic, payload should be [UnsubscribeRequest]
//!
//! Events that are emitted:
//! - `dooot`: A topic subscribed to emits a dooot, payload is [DoootMessage]
//! - `subscribed`: Successfully subscribed to a topic, payload is the topic name
//! - `unsubscribed`: Successfully unsubscribed from a topic, payload is the topic name
//! - `error`: An error occurred, payload is a string describing the error
//!
//! The format for topics is `<dooot_name>.<field name>.<field value>`. For instance,
//! a `SolTransfer` dooot with a `source` field of `123` would have a topic of `SolTransfer.source.123`.
//! Some dooots are also published as general topics as just `<dooot_name>`.
//!
//! For specific dooots, and their fields exposed as topics, see the [step_ingestooor_sdk::dooot] module.
use std::{future::IntoFuture, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use data_writer::ApiLog;
use evalexpr::Node;
use handlers::{connect::handle_connect, transaction::transaction_handler};
use hmac::{Hmac, Mac};

use indexer_rabbitmq::lapin::{options::QueueDeclareOptions, types::FieldTable};
use middleware::auth::auth_middleware;
use socketioxide::SocketIoBuilder;
use step_ingestooor_engine::rabbit_factory;

#[doc(inline)]
pub use messages::*;
use tokio::sync::mpsc;

use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    timeout::TimeoutLayer,
};
use transaction_receiver::TransactionRequest;

#[doc(hidden)]
mod auth;
#[doc(hidden)]
mod data_writer;
#[doc(hidden)]
mod dooot_receiver;
#[doc(hidden)]
mod handlers;
#[doc(hidden)]
mod messages;
#[doc(hidden)]
mod middleware;
#[doc(hidden)]
mod state;
#[doc(hidden)]
mod transaction_receiver;

type TopicFilterMap = DashMap<String, DashMap<String, Option<Node>>>;

/// The path to the socket.io namespace that handles dooot subscriptions
pub const SCHEMA_SOCKETIO_PATH: &str = "/dooots";
/// the path for a healthcheck endpoint
pub const HEATHCHECK_PATH: &str = "/healthcheck";
/// the path for a transaction endpoint
pub const TXN_PATH: &str = "/transaction";
/// The address and port to bind the socket server to
pub const BIND_ADDR_PORT: &str = "0.0.0.0:3000";

/// The arguments for the broadcastooor. These can be passed as arguments or environment variables.
#[derive(Parser, PartialEq, Debug)]
pub struct BroadcastooorArgs {
    /// The address of an AMQP server to connect to
    #[clap(long, env)]
    pub rabbitmq_url: String,

    /// The exchange to use
    #[clap(long, env)]
    pub rabbitmq_dooot_exchange: String,

    /// The txn exchange to use
    #[clap(long, env)]
    pub rabbitmq_txn_exchange: String,

    /// The rabbitMQ prefetch count to use when reading from queues
    /// This loosely translates to # simultaneous messages being processed
    #[clap(long, env)]
    pub rabbitmq_prefetch: Option<u16>,

    /// The domains to allow connections from
    #[clap(long, env)]
    pub whitelisted_origins: String,

    /// The secret to use for JWTs
    #[clap(long, env)]
    pub jwt_secret: String,

    /// The secret to use for JWTs
    #[clap(long, env, default_value = "false", parse(try_from_str))]
    pub no_auth: bool,

    /// Should we log to the database
    #[clap(long, env, default_value = "true", parse(try_from_str))]
    pub no_db_log: bool,

    /// The database connection string for logging
    #[clap(long, env, required_if_eq("no_db_log", "false"))]
    pub database_con_string: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub global_txn_sender: Arc<mpsc::Sender<TransactionRequest>>,
}

#[doc(hidden)]
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    //setup metrics
    if let Err(e) = step_common_rust::init_metrics("Broadcastooor").await {
        log::error!("Error initializing metrics: {}", e);
    }

    let args = BroadcastooorArgs::parse();

    //if no auth, log an error letting the user know
    if args.no_auth {
        log::error!("No auth is enabled, this is a security risk");
    }

    //if no auth, log an error letting the user know
    if args.no_db_log {
        log::error!("No database logging, only use for local testing");
    }

    //database thread setup
    //quick db test first
    if !args.no_db_log {
        log::debug!("database logging enabled, testing connection");
        let (_, _) = tokio_postgres::connect(
            args.database_con_string.as_ref().unwrap(),
            tokio_postgres::NoTls,
        )
        .await?;
        log::debug!("database connection successful");
    }
    let (api_log_sender, api_log_receiver) = tokio::sync::mpsc::unbounded_channel::<ApiLog>();
    let db_thread = tokio::spawn(data_writer::create_database_writer_task(
        api_log_receiver,
        if args.no_db_log {
            None
        } else {
            args.database_con_string
        },
    ));

    //rabbit setup
    let connection = rabbit_factory::amqp_connect(args.rabbitmq_url, "broadcastooor").await?;
    let channel = connection.create_channel().await?;
    let channel = Arc::new(channel);
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
            arguments.clone(),
        )
        .await?;
    channel
        .queue_bind(
            queue.name().as_str(),
            &args.rabbitmq_dooot_exchange,
            "#",
            Default::default(),
            Default::default(),
        )
        .await?;
    // create another temp queue to listen to txn feed
    // also capped at 10k
    let txn_queue = channel
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
            txn_queue.name().as_str(),
            &args.rabbitmq_txn_exchange,
            "#",
            Default::default(),
            Default::default(),
        )
        .await?;

    //read the allowed origins
    let whitelisted_origins: Vec<_> = args
        .whitelisted_origins
        .split(',')
        .map(|a| a.to_string())
        .collect();

    log::debug!(
        "Allowed domains configured using allowed_domains: {:?}",
        whitelisted_origins
    );

    let whitelisted_origins_clone = whitelisted_origins.clone();
    let allow_origin_predicate = AllowOrigin::predicate(move |origin, _| {
        let origin = origin.to_str().unwrap();
        whitelisted_origins_clone.iter().any(|a| origin.contains(a))
    });

    //build the tower layers
    let cors_layer = CorsLayer::new()
        //allow requests from origins matching
        .allow_origin(allow_origin_predicate);

    let (txn_tx, txn_rx) = mpsc::channel::<TransactionRequest>(1024);

    //create state for the socket server to have
    let state = state::BroadcastooorState::new(
        whitelisted_origins,
        Hmac::new_from_slice(args.jwt_secret.as_bytes())?,
        args.no_auth,
        api_log_sender,
        Arc::new(txn_tx),
    );

    //socket server setup
    let (io_layer, io) = SocketIoBuilder::new()
        .with_state(state.clone())
        .build_layer();

    //handle the connection event, which does auth & sets up event listeners
    io.ns(SCHEMA_SOCKETIO_PATH, handle_connect);

    let app = axum::Router::new()
        //transaction grabbing
        .route(TXN_PATH, axum::routing::get(transaction_handler))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        //socketio
        .layer(io_layer)
        //cors
        .layer(cors_layer)
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        //healthcheck for aws
        .route(HEATHCHECK_PATH, axum::routing::get(|| async { "ok" }));

    //create a thread that uses rabbit to listen and publish dooots
    let dooot_thread = tokio::spawn(dooot_receiver::run_rabbit_thread(
        channel.clone(),
        queue,
        args.rabbitmq_prefetch.unwrap_or(64_u16),
        io,
    ));

    let txn_thread = tokio::spawn(transaction_receiver::run_txn_reader_thread(
        channel,
        txn_queue,
        args.rabbitmq_prefetch.unwrap_or(64_u16),
        txn_rx,
    ));

    //create the socket server
    let listener = tokio::net::TcpListener::bind(BIND_ADDR_PORT).await.unwrap();
    let app_thread = axum::serve(listener, app).into_future();

    log::info!("started listening on {}", BIND_ADDR_PORT);

    //wait for either thread to fail
    tokio::select! {
        e = dooot_thread => {
            log::error!("publisher thread exited {:?}", e);
        }
        e = txn_thread => {
            log::error!("txn thread exited {:?}", e);
        }
        e = app_thread => {
            log::error!("app thread exited {:?}", e);
        }
        e = db_thread => {
            log::error!("db thread exited {:?}", e);
        }
    };

    log::error!("broadcastooor exiting");
    Ok(())
}
