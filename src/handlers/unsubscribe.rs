use dashmap::DashMap;
use evalexpr::Node;
use log::{debug, error, info, warn};
use socketioxide::extract::{SocketRef, TryData};

use crate::messages::UnsubscribeRequest;

pub fn handle_unsubscribe(s: SocketRef, msg: TryData<String>) {
    let TryData(msg) = msg;
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };
    let msg = match serde_json::from_str::<UnsubscribeRequest>(&msg) {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse unsubscribe request: {}", e);
            s.emit("error", format!("unsubscribe error: {}", e)).ok();
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
                return;
            }
            debug!("unsubscribed from {} with filter {}", msg.topic, filter_id);
        } else {
            room_filters.remove("");
            debug!("unsubscribed from {}", msg.topic);
        }
        if room_filters.is_empty() {
            //if there are no more filters for the room, leave the room and delete the room filters entry
            s.leave(msg.topic.clone()).ok();
            all_filters.remove(&msg.topic);
        }
    } else {
        warn!("somehow no room filters for {}", msg.topic);
        //register a leave, but not sure how in this state
        s.leave(msg.topic.clone()).ok();
    }
    //notify the client that they have unsubscribed
    s.emit("unsubscribed", msg.topic).ok();
}
