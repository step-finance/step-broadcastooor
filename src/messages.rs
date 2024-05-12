

#[derive(serde::Deserialize)]
pub struct Filter {
    id: String,
    expression: String,
}

#[derive(serde::Deserialize)]
pub struct SubscribeRequest {
    topic: String,
    filter: Option<Filter>,
}

#[derive(serde::Deserialize)]
pub struct UnsubscribeRequest {
    topic: String,
    filter_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Message {
    topic: String,
    filter_id: Option<String>,
    schema: Schema,
}