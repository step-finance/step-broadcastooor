use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tokio::sync::oneshot;

use crate::{state::BroadcastooorState, transaction_receiver::TransactionRequest};

#[derive(Serialize, Deserialize)]
pub struct TransactionRequestParams {
    pub filter: String,
}

pub async fn transaction_handler(
    State(BroadcastooorState {
        global_txn_sender, ..
    }): State<BroadcastooorState>,
    Query(params): Query<TransactionRequestParams>,
) -> Result<Json<Value>, axum::http::StatusCode> {
    let (tx, rx) = oneshot::channel::<Vec<u8>>();
    let filters = params
        .filter
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let req = TransactionRequest {
        response_sender: tx,
        filters,
    };

    global_txn_sender.send(req).await.map_err(|e| {
        log::error!("Failed to send transaction request: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = rx.await.map_err(|e| {
        log::error!("Failed to receive transaction response: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::from_slice(&response).map_err(|e| {
        log::error!("Failed to parse transaction response: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?))
}
