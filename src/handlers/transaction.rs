use axum::{
    extract::{Query, State},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tokio::sync::oneshot;

use crate::{state::BroadcastooorState, transaction_receiver::TransactionRequest};

#[derive(Serialize, Deserialize)]
pub struct TransactionRequestParams {
    pub filter: String,
    pub num_txns: Option<u8>,
}

pub async fn transaction_handler(
    State(BroadcastooorState {
        global_txn_sender, ..
    }): State<BroadcastooorState>,
    Query(params): Query<TransactionRequestParams>,
) -> Result<&'static str, axum::http::StatusCode> {
    let (tx, rx) = oneshot::channel::<Vec<Vec<u8>>>();
    let filters = params
        .filter
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let num_txns = params.num_txns.unwrap_or(1);
    if num_txns > 25 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    let req = TransactionRequest {
        response_sender: tx,
        filters,
        num_txns,
        transactions: Vec::with_capacity(num_txns as usize),
    };

    global_txn_sender.send(req).await.map_err(|e| {
        log::error!("Failed to send transaction request: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = rx.await.map_err(|e| {
        log::error!("Failed to receive transaction response: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut buf = Vec::new();
    let zip_obj = zip::ZipWriter::new(&mut buf);

    Ok("TEST")
}
