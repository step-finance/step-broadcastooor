use std::sync::Arc;

use futures_util::StreamExt;
use indexer_rabbitmq::lapin::{
    options::{BasicConsumeOptions, BasicQosOptions},
    Channel, Queue,
};
use serde_json::Value;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    oneshot,
};

pub type TransactionRequestFilters = Vec<String>;

pub struct TransactionRequest {
    pub response_sender: oneshot::Sender<Vec<Vec<u8>>>,
    pub filters: TransactionRequestFilters,
    pub num_txns: u8,
    pub transactions: Vec<Vec<u8>>,
}

pub async fn run_txn_reader_thread(
    channel: Arc<Channel>,
    queue: Queue,
    prefetch: u16,
    mut txn_request_receiver: mpsc::Receiver<TransactionRequest>,
) {
    let queue_name = queue.name().as_str();
    log::warn!("Starting transaction reader on queue: {}", queue_name);
    //set the prefetch on the channel
    channel
        .basic_qos(prefetch, BasicQosOptions::default())
        .await
        .expect("failed to set qos");

    //create a message consumer
    let mut consumer = channel
        .basic_consume(
            queue.name().as_str(),
            "broadcastooor-txn",
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

    let mut txn_requests = Vec::new();

    //process messages async (neverending loop)
    while let Some(message_result) = consumer.next().await {
        // Look for a new txn request, if found, add it to the list
        let txn_req = txn_request_receiver.try_recv();
        match txn_req {
            Ok(txn_req) => {
                txn_requests.push(txn_req);
            }
            Err(e) => match e {
                TryRecvError::Empty => {}
                TryRecvError::Disconnected => {
                    panic!("Transaction request receiver poisoned");
                }
            },
        }

        txn_requests.retain(|req| !req.response_sender.is_closed());

        if txn_requests.is_empty() {
            continue;
        }

        if let Ok(delivery) = message_result {
            let data = delivery.data;
            let Ok(txn_json) = serde_json::from_slice::<Value>(&data) else {
                // Somehow this isn't json... whatever
                continue;
            };

            let Some(account_keys) = txn_json
                .get("transaction")
                .and_then(|val| val.get("message"))
                .and_then(|val| val.get("accountKeys"))
                .and_then(|keys| keys.as_array())
                .and_then(|v| {
                    v.iter()
                        .map(|value| value.get("pubkey").and_then(|pk| pk.as_str()))
                        .collect::<Option<Vec<_>>>()
                })
            else {
                // No accounts, skip
                // Probably SlotStats
                continue;
            };

            let mut remaining_requests = Vec::new();

            for mut req in txn_requests.into_iter() {
                let filters = &req.filters;
                let acct_exists = account_keys
                    .iter()
                    .any(|&key| filters.contains(&key.to_string()));
                if acct_exists {
                    req.transactions.push(data.clone());
                    if req.transactions.len() >= req.num_txns as usize {
                        // Valid match, send the data, and let the request drop out
                        req.response_sender.send(req.transactions).unwrap();
                        continue;
                    }
                }

                // Request has not fulfilled yet, keep it
                remaining_requests.push(req);
            }

            // CRITICAL: Replace txn_requests with the remaining requests
            txn_requests = remaining_requests;
        }
    }
}
