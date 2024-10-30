use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use http::StatusCode;
use jwt::VerifyWithKey;
use serde_json::json;

use crate::{
    auth::claims::UserJWT, handlers::connect::ConnectedUserInfo, state::BroadcastooorState,
};

pub async fn auth_middleware(
    State(state): State<BroadcastooorState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut user = ConnectedUserInfo::default();
    if state.no_auth {
        req.extensions_mut().insert(user);
        return Ok(next.run(req).await);
    }

    let headers = req.headers();
    // Add IP to user info
    let forwarded_for = headers.get("X-Forwarded-For").and_then(|v| {
        v.to_str()
            .ok()
            .map(|a| String::from(a.split(',').next().unwrap()))
    });
    user.ip_address = forwarded_for;

    // Check origin
    let whitelisted_origins = &state.whitelisted_origins;
    let origin = headers.get("Origin").and_then(|v| v.to_str().ok());
    if let Some(origin) = origin {
        user.origin = Some(origin.to_owned());
        if whitelisted_origins.iter().any(|wl| origin.contains(wl)) {
            req.extensions_mut().insert(user);
            return Ok(next.run(req).await);
        }
    }

    // Check JWT
    if let Some(auth) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        let mut sp = auth.split_whitespace();
        let is_bearer = sp.next().is_some_and(|b| b == "Bearer");
        if !is_bearer {
            return Err(StatusCode::BAD_REQUEST);
        }
        if let Some(token) = sp.next().map(|t| t.to_string()) {
            let user_jwt_info: UserJWT = token.verify_with_key(&state.jwt_secret).map_err(|e| {
                log::error!("JWT verification failed: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            user.claims = Some(user_jwt_info.clone());

            let now = chrono::Utc::now().timestamp();
            if user_jwt_info.exp > now
                && user_jwt_info.iat < now
                && user_jwt_info.has_role(&"stream".to_string(), None)
            {
                req.extensions_mut().insert(user);
                return Ok(next.run(req).await);
            }
        };
    };

    state.send_log_with_message(
        &user,
        "auth-fail",
        Some(&json!({
            "error": "Unauthorized",
            "message": "Unauthorized request from user",
            "user": user,
        })),
        200,
        user.origin.clone(),
    );
    log::error!("Unauthorized request from {:?}", user);

    Err(StatusCode::UNAUTHORIZED)
}
