use std::sync::Arc;

use evalexpr::build_operator_tree;
use log::{debug, error, info};
use metrics_cloudwatch::metrics;
use serde_json::json;
use socketioxide::{
    adapter::Room,
    extract::{Extension, SocketRef, State, TryData},
};

use crate::{messages::SubscribeRequest, state::BroadcastooorState, TopicFilterMap};

use super::connect::ConnectedUserInfo;

pub fn handle_subscribe(
    s: SocketRef,
    all_filters: Extension<Arc<TopicFilterMap>>,
    user: Extension<Arc<ConnectedUserInfo>>,
    state: State<BroadcastooorState>,
    msg: TryData<SubscribeRequest>,
) {
    debug!("received subscribe request with data: {:?}", msg.0);
    let msg: SubscribeRequest = match msg {
        TryData(Ok(msg)) => msg,
        TryData(Err(e)) => {
            error!(
                "Failed to parse subscribe request into SubscribeRequest: {}",
                e
            );
            if let Err(e) = s.emit(
                "serverError",
                format!(
                    "Failed to parse subscribe request into SubscribeRequest: {}",
                    e
                ),
            ) {
                error!("failed to emit serverError: {}", e);
            }
            state.send_log_with_message(&user, "subscribe", Some(&e.to_string()), 500, None);
            return;
        }
    };

    info!(
        "received subscribe for {} with filter {:?}",
        msg.topic, msg.filter
    );

    //get the room filters or create a new one
    let room_filters = all_filters.entry(msg.topic.clone()).or_default();

    //add filter for the room
    if let Some(filter) = msg.filter.clone() {
        match build_operator_tree(&filter.expression) {
            Ok(tree) => {
                //insert the filter into the room filters
                room_filters.value().insert(filter.id.clone(), Some(tree));
            }
            Err(e) => {
                error!("failed to parse filter expression: {}", e);
                if let Err(e) = s.emit("serverError", format!("subscribe error: {}", e)) {
                    error!("failed to emit serverError: {}", e);
                }
                state.send_log_with_message(
                    &user,
                    "subscribe",
                    Some(&json!({
                        "error": "failed to parse filter expression",
                        "message": msg,
                    })),
                    500,
                    None,
                );
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
    if let Err(e) = s.join(Room::Owned(msg.topic.clone())) {
        error!("failed to join room: {}", e);
    }

    //notify the client that they have subscribed
    if let Err(e) = s.emit(
        "subscribed",
        [(
            msg.topic.clone(),
            msg.filter
                .as_ref()
                .map(|f| f.id.clone())
                .unwrap_or_default(),
        )],
    ) {
        error!("failed to emit subscribed: {}", e);
    }

    state.send_log_with_message(&user, "subscribe", Some(&msg), 200, None);

    //metrics
    {
        let mut topic_parts = msg.topic.split('.');
        let dooot_name = topic_parts.next().unwrap_or_default().to_string();
        let field_name = topic_parts.next().unwrap_or_default().to_string();
        // let labels = [("Dooot", dooot_name), ("Field", field_name)];
        // metrics::increment_gauge!("TotalSubscriptions", 1.0, &labels);
        metrics::increment_gauge!("CurrentSubscriptions", 1.0,
            "Dooot" => dooot_name.clone(),
            "Field" => field_name.clone(),
        );
        metrics::increment_counter!("TotalSubscriptions",
            "Dooot" => dooot_name,
            "Field" => field_name,
        );
    }
}
