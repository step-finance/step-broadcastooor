use step_ingestooor_sdk::schema::Schema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Filter {
    pub id: String,
    pub expression: String,
}

#[derive(Deserialize)]
pub struct SubscribeRequest {
    pub topic: String,
    pub filter: Option<Filter>,
}

#[derive(Deserialize)]
pub struct UnsubscribeRequest {
    pub topic: String,
    pub filter_id: Option<String>,
}

#[derive(Serialize)]
pub struct Message {
    pub topic: String,
    pub filter_id: Option<String>,
    pub schema: Schema,
}