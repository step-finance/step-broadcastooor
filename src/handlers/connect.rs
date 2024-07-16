use std::sync::Arc;

use jwt::{Header, Token, VerifyWithKey as _};
use metrics_cloudwatch::metrics;
use socketioxide::extract::{SocketRef, State, TryData};

use crate::{
    auth::{claims, AuthData},
    handlers::{subscribe::handle_subscribe, unsubscribe::handle_unsubscribe},
    state::BroadcastooorState,
    TopicFilterMap,
};

#[derive(Clone, Debug)]
pub struct ConnectedUserInfo {
    pub origin: Option<String>,
    pub claims: Option<claims::Root>,
}

pub fn handle_connect(
    s: SocketRef,
    State(state): State<Arc<BroadcastooorState>>,
    TryData(auth): TryData<AuthData>,
) {
    let auth = get_auth(auth, &s);

    //get the origin
    let headers = &s.req_parts().headers;
    let origin = headers
        .get("Origin")
        .and_then(|v| v.to_str().ok().map(String::from));

    //check auth
    let auth_ok = auth_check(&auth, &origin, &state);
    if !state.no_auth && !auth_ok.0 {
        log::info!("Invalid auth");
        s.emit("serverError", "Invalid auth").ok();
        s.disconnect().ok();
        return;
    }

    metrics::increment_counter!("TotalConnections");
    metrics::increment_gauge!("CurrentConnections", 1.0);
    log::info!("Client connected");

    //throw the claims data in the extensions
    let user = ConnectedUserInfo {
        origin,
        claims: auth_ok.1,
    };
    s.extensions.insert(Arc::new(user));

    //create the empty filter map on all sockets
    let filters = TopicFilterMap::new();
    s.extensions.insert(Arc::new(filters));

    //create the handlers
    s.on_disconnect(|| {
        metrics::decrement_gauge!("CurrentConnections", 1.0);
        log::info!("Client disconnected");
    });
    s.on("subscribe", handle_subscribe);
    s.on("unsubscribe", handle_unsubscribe);
}

//uses the auth passed in, or tries to get it from the headers
#[inline]
fn get_auth(auth: Result<AuthData, serde_json::Error>, s: &SocketRef) -> Option<AuthData> {
    let mut auth = auth.ok();
    if auth.is_none() {
        let headers = &s.req_parts().headers;
        let auth_header = headers.get("Authorization");
        if let Some(auth_header) = auth_header {
            let auth_header = auth_header.to_str().unwrap();
            let auth_header = auth_header.split_whitespace().collect::<Vec<&str>>();
            if auth_header.len() == 2 && auth_header[0] == "Bearer" {
                let token = auth_header[1];
                auth = Some(AuthData {
                    token: token.to_string(),
                });
            }
        }
    }
    auth
}

//validate the jwt token or origin
#[inline]
fn auth_check(
    auth: &Option<AuthData>,
    origin: &Option<String>,
    state: &BroadcastooorState,
) -> (bool, Option<claims::Root>) {
    if let Some(auth) = auth {
        let claims: Result<Token<Header, claims::Root, _>, _> =
            auth.token.as_str().verify_with_key(&state.jwt_secret);
        if let Ok(claims) = claims {
            let (_header, claims) = claims.into();
            let now = chrono::Utc::now().timestamp();
            if claims.exp > now && claims.iat < now && claims.has_role(&"stream".to_string(), None)
            {
                return (true, Some(claims));
            }
        }
    }
    if let Some(origin) = origin {
        if state.whitelisted_origins.iter().any(|a| origin.contains(a)) {
            return (true, None);
        }
    }
    (false, None)
}
