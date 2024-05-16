use step_ingestooor_sdk::schema::Schema;

#[derive(serde::Deserialize)]
pub struct Filter {
    pub id: String,
    pub expression: String,
}

#[derive(serde::Deserialize)]
pub struct SubscribeRequest {
    pub topic: String,
    pub filter: Option<Filter>,
}

#[derive(serde::Deserialize)]
pub struct UnsubscribeRequest {
    pub topic: String,
    pub filter_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Message {
    pub topic: String,
    pub filter_id: Option<String>,
    pub schema: Schema,
}
