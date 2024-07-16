use hmac::Hmac;
use sha2::Sha256;

pub struct BroadcastooorState {
    pub whitelisted_origins: Vec<String>,
    pub jwt_secret: Hmac<Sha256>,
    pub no_auth: bool,
}

impl BroadcastooorState {
    pub fn new(whitelisted_origins: Vec<String>, jwt_secret: Hmac<Sha256>, no_auth: bool) -> Self {
        Self {
            whitelisted_origins,
            jwt_secret,
            no_auth,
        }
    }
}
