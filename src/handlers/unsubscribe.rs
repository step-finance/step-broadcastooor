use std::sync::Arc;

use log::{debug, error, info, warn};
use metrics_cloudwatch::metrics;
use socketioxide::extract::{Extension, SocketRef, TryData};

use crate::{messages::UnsubscribeRequest, TopicFilterMap};

pub fn handle_unsubscribe(
    s: SocketRef,
    all_filters: Extension<Arc<TopicFilterMap>>,
    msg: TryData<UnsubscribeRequest>,
) {
    let msg: UnsubscribeRequest = match msg {
        TryData(Ok(msg)) => msg,
        TryData(Err(e)) => {
            error!(
                "Failed to parse unsubscribe request into UnsubscribeRequest: {}",
                e
            );
            if let Err(e) = s.emit(
                "serverError",
                format!(
                    "Failed to parse unsubscribe request into UnsubscribeRequest: {}",
                    e
                ),
            ) {
                error!("failed to emit serverError: {}", e);
            }
            return;
        }
    };

    info!(
        "received unsubscribe for {} filter {:?}",
        msg.topic, msg.filter_id
    );

    let mut empty_room = false;
    //grab the room filters
    if let Some(room_filters) = all_filters.get_mut(&msg.topic) {
        //leave the filter for the room
        if let Some(filter_id) = msg.filter_id.clone() {
            let removed_filter = room_filters.remove(&filter_id);
            if removed_filter.is_none() {
                debug!(
                    "no filter found for {} with filter {}",
                    msg.topic, filter_id
                );
                if let Err(e) = s.emit("serverError", "filter not found") {
                    error!("failed to emit serverError: {}", e);
                }
                return;
            }
            debug!("unsubscribed from {} filter {}", msg.topic, filter_id);
        } else {
            if room_filters.remove("").is_none() {
                debug!("generic room filter not found for {}", msg.topic);
                if let Err(e) = s.emit("serverError", "not subscribed genericly to that topic") {
                    error!("failed to emit serverError: {}", e);
                }
                return;
            }
            debug!("unsubscribed from {}", msg.topic);
        }
        if room_filters.is_empty() {
            //if there are no more filters for the room, leave the room and delete the room filters entry
            if let Err(e) = s.leave(msg.topic.clone()) {
                error!("failed to leave room: {}", e);
            }
            empty_room = true;
        }
    } else {
        //client isn't subscribed to the room
        warn!("no room filters for {}", msg.topic);
        //register a leave just in case, but not sure how in this state
        if let Err(e) = s.leave(msg.topic.clone()) {
            error!("failed to leave room (wasnt subscribed anyhow?): {}", e);
        }
    }

    if empty_room {
        all_filters.remove(&msg.topic);
    }

    //notify the client that they have unsubscribed
    if let Err(e) = s.emit(
        "unsubscribed",
        [(msg.topic.clone(), msg.filter_id.unwrap_or_default())],
    ) {
        error!("failed to emit unsubscribed: {}", e);
    }

    //metrics
    {
        let mut topic_parts = msg.topic.split('.');
        let schema_name = topic_parts.next().unwrap_or_default().to_string();
        let field_name = topic_parts.next().unwrap_or_default().to_string();
        // let labels = [("Schema", schema_name), ("Field", field_name)];
        // metrics::increment_gauge!("TotalSubscriptions", 1.0, &labels);
        metrics::decrement_gauge!("CurrentSubscriptions", 1.0,
            "Schema" => schema_name,
            "Field" => field_name,
        );
    }
}
