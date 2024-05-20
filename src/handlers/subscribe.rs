use dashmap::DashMap;
use evalexpr::{build_operator_tree, Node};
use log::{debug, error, info};
use socketioxide::{
    adapter::Room,
    extract::{SocketRef, TryData},
};

use crate::messages::SubscribeRequest;

pub fn handle_subscribe(s: SocketRef, msg: TryData<String>) {
    let TryData::<String>(msg) = msg;
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };
    let msg = match serde_json::from_str::<SubscribeRequest>(&msg) {
        Ok(msg) => msg,
        Err(e) => {
            error!("failed to parse subscribe request: {}", e);
            s.emit("error", format!("subscribe error: {}", e)).ok();
            return;
        }
    };

    info!("received subscribe for {}", msg.topic);

    //get a reference to filters on the socket
    let all_filters = s
        .extensions
        .get_mut::<DashMap<String, DashMap<String, Option<Node>>>>()
        .unwrap();
    //get the room filters or create a new one
    let room_filters = all_filters
        .entry(msg.topic.clone())
        .or_insert_with(DashMap::<String, Option<Node>>::new);

    //add filter for the room
    if let Some(filter) = msg.filter {
        match build_operator_tree(&filter.expression) {
            Ok(tree) => {
                //insert the filter into the room filters
                room_filters.value().insert(filter.id.clone(), Some(tree));
            }
            Err(e) => {
                error!("failed to parse filter expression: {}", e);
                s.emit("error", format!("subscribe error: {}", e)).ok();
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
    s.join(Room::Owned(msg.topic.clone())).ok();

    //notify the client that they have subscribed
    s.emit("subscribed", msg.topic).ok();
}
