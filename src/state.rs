use tokio::sync::mpsc;

use hmac::Hmac;
use sha2::Sha256;

use crate::data_writer::ApiLog;

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
}
