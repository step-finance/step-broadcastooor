use serde::{Deserialize, Serialize};
use step_ingestooor_sdk::schema::Schema;

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
pub struct Message<'a> {
    pub topic: String,
    pub filter_id: Option<&'a String>,
    pub schema: Schema,
}
