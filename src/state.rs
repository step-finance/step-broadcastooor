use serde::Serialize;
use tokio::sync::mpsc;

use hmac::Hmac;
use sha2::Sha256;

use crate::{data_writer::ApiLog, handlers::connect::ConnectedUserInfo};

pub struct BroadcastooorState {
    pub whitelisted_origins: Vec<String>,
    pub jwt_secret: Hmac<Sha256>,
    pub no_auth: bool,
    pub db_log: mpsc::UnboundedSender<ApiLog>,
}

impl BroadcastooorState {
    pub fn new(
        whitelisted_origins: Vec<String>,
        jwt_secret: Hmac<Sha256>,
        no_auth: bool,
        db_log: mpsc::UnboundedSender<ApiLog>,
    ) -> Self {
        Self {
            whitelisted_origins,
            jwt_secret,
            no_auth,
            db_log,
        }
    }
    pub fn send_log_with_message<T: Serialize>(
        &self,
        user: &ConnectedUserInfo,
        action: &str,
        message: Option<&T>,
        status: i32,
        referer: Option<String>,
    ) {
        let mut api_log = ApiLog::from_user(user);
        api_log.query_params = message.map(serde_json::to_value).transpose().ok().flatten();
        api_log.status_code = Some(status);
        api_log.endpoint = action.to_string();
        api_log.referer = referer;
        self.db_log.send(api_log).ok();
    }
    pub fn send_log(
        &self,
        user: &ConnectedUserInfo,
        action: &str,
        status: i32,
        referer: Option<String>,
    ) {
        let mut api_log = ApiLog::from_user(user);
        api_log.status_code = Some(status);
        api_log.endpoint = action.to_string();
        api_log.referer = referer;
        self.db_log.send(api_log).ok();
    }
}
