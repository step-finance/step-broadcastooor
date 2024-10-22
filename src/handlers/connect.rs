use std::sync::Arc;

use metrics_cloudwatch::metrics;
use socketioxide::extract::{SocketRef, State};

use crate::{
    auth::claims,
    handlers::{subscribe::handle_subscribe, unsubscribe::handle_unsubscribe},
    state::BroadcastooorState,
    TopicFilterMap,
};

#[derive(Clone, Default, Debug)]
pub struct ConnectedUserInfo {
    pub ip_address: Option<String>,
    pub origin: Option<String>,
    pub claims: Option<claims::UserJWT>,
}

pub fn handle_connect(s: SocketRef, State(state): State<Arc<BroadcastooorState>>) {
    // Get the auth data from the request
    let parts = s.req_parts();
    let extensions = &s.extensions;
    let Some(user) = extensions.get::<ConnectedUserInfo>() else {
        log::error!("No user info found in extensions");
        s.emit("serverError", "No user info found in extensions")
            .ok();
        s.disconnect().ok();
        return;
    };

    //get the origin
    let headers = &parts.headers;
    let origin = headers
        .get("Origin")
        .and_then(|v| v.to_str().ok().map(String::from));
    log::debug!("Origin: {:?}", origin);

    //get the forwarded ip
    let forwarded_for = headers.get("X-Forwarded-For").and_then(|v| {
        v.to_str()
            .ok()
            .map(|a| String::from(a.split(',').next().unwrap()))
    });
    log::debug!("X-Forwarded-For: {:?}", forwarded_for);

    metrics::increment_counter!("TotalConnections");
    metrics::increment_gauge!("CurrentConnections", 1.0);
    log::info!("Client connected");

    //create the empty filter map on all sockets
    let filters = TopicFilterMap::new();
    s.extensions.insert(Arc::new(filters));

    //create the handlers
    let user_ref = user.clone();
    let state_ref = state.clone();
    let origin_ref = origin.clone();
    s.on_disconnect(move || {
        //send log on disconnect
        state_ref.send_log(&user_ref, "disconnect", 200, origin_ref);

        metrics::decrement_gauge!("CurrentConnections", 1.0);
        log::info!("Client disconnected");
    });
    s.on("subscribe", handle_subscribe);
    s.on("unsubscribe", handle_unsubscribe);

    //send log for connect
    state.send_log(&user, "connect", 200, origin);
}
