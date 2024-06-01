use dashmap::DashMap;
use evalexpr::Node;
use log::{debug, error, info, warn};
use socketioxide::extract::{SocketRef, TryData};
use metrics_cloudwatch::metrics;

use crate::messages::UnsubscribeRequest;

pub fn handle_unsubscribe(s: SocketRef, msg: TryData<UnsubscribeRequest>) {
    let msg: UnsubscribeRequest = match msg {
        TryData(Ok(msg)) => msg,
        TryData(Err(e)) => {
            error!("Failed to parse unsubscribe request into UnsubscribeRequest: {}", e);
            s.emit("serverError", format!("Failed to parse unsubscribe request into UnsubscribeRequest: {}", e)).ok();
            return;
        }
    };

    info!("received unsubscribe for {}", msg.topic);

    //get a reference to filters on the socket
    let all_filters = s
        .extensions
        .get_mut::<DashMap<String, DashMap<String, Option<Node>>>>()
        .unwrap();

    //grab the room filters
    if let Some(room_filters) = all_filters.get_mut(&msg.topic) {
        //leave the filter for the room
        if let Some(filter_id) = msg.filter_id {
            if room_filters.remove(&filter_id).is_none() {
                debug!(
                    "no filter found for {} with filter {}",
                    msg.topic, filter_id
                );
                s.emit("serverError", "filter not found").ok();
                return;
            }
            debug!("unsubscribed from {} filter {}", msg.topic, filter_id);
        } else {
            if room_filters.remove("").is_none() {
                debug!("generic room filter not found for {}", msg.topic);
                s.emit("serverError", "not subscribed genericly to that topic").ok();
                return;
            }
            debug!("unsubscribed from {}", msg.topic);
        }
        if room_filters.is_empty() {
            //if there are no more filters for the room, leave the room and delete the room filters entry
            s.leave(msg.topic.clone()).ok();
            all_filters.remove(&msg.topic);
        }
    } else {
        //client isn't subscribed to the room
        warn!("no room filters for {}", msg.topic);
        //register a leave just in case, but not sure how in this state
        s.leave(msg.topic.clone()).ok();
    }
    //notify the client that they have unsubscribed
    s.emit("unsubscribed", msg.topic.clone()).ok();

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
