use std::io::{Cursor, Write};

use axum::extract::{Query, State};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tokio::sync::oneshot;
use zip::write::SimpleFileOptions;

use crate::{state::BroadcastooorState, transaction_receiver::TransactionRequest};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequestParams {
    pub filter: String,
    pub num_txns: Option<u8>,
}

pub async fn transaction_handler(
    State(BroadcastooorState {
        global_txn_sender, ..
    }): State<BroadcastooorState>,
    Query(params): Query<TransactionRequestParams>,
) -> Result<(HeaderMap, Vec<u8>), axum::http::StatusCode> {
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

    let mut response = rx.await.map_err(|e| {
        log::error!("Failed to receive transaction response: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if response.len() == 1 {
        let mut header_map = HeaderMap::new();
        header_map.insert("Content-Type", "application/json".parse().unwrap());
        let data = response
            .drain(..)
            .next()
            .expect("UNREACHABLE - response.len() == 1");
        return Ok((header_map, data));
    }

    let mut buf = Cursor::new(Vec::new());
    let mut zip_obj = zip::ZipWriter::new(&mut buf);
    for json_bytes in response.drain(..) {
        let json_val = serde_json::from_slice::<Value>(&json_bytes).map_err(|e| {
            log::error!("Failed to serialize transaction response: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let sig = json_val
            .get("transaction")
            .and_then(|txn| txn.get("signatures"))
            .and_then(|sigs| sigs.as_array())
            .and_then(|sigs| sigs.first())
            .and_then(|sig| sig.as_str())
            .ok_or_else(|| {
                log::error!("Failed to get signature from transaction response");
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let file_name = format!("txn-{sig}.json");
        zip_obj
            .start_file(file_name, SimpleFileOptions::default())
            .map_err(|e| {
                log::error!("Failed to start file in zip: {}", e);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            })?;

        zip_obj.write(&json_bytes).map_err(|e| {
            log::error!("Failed to write to zip: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    zip_obj.finish().map_err(|e| {
        log::error!("Failed to finish zip: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    drop(zip_obj);

    let zip_file_bytes = buf.into_inner();

    let mut header_map = HeaderMap::new();
    header_map.insert("Content-Type", "application/zip".parse().unwrap());

    Ok((header_map, zip_file_bytes))
}
