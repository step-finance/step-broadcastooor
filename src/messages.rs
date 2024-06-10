use serde::{Deserialize, Serialize};
use step_ingestooor_sdk::schema::Schema;
use ts_rs::TS;

/// A filter allows the server to filter out schemas based on expressions.
#[derive(Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Filter {
    /// An identifier for the filter.  This is used to unsubscribe from a specific filter.  
    /// If two filters have the same filter identifier the behavior is undefined. Don't do that.
    pub id: String,
    /// The expression to filter on.
    /// The schema's fields of types `i64`, `u64`, `f64`, `String`, and `bool` are available in
    /// the expression as their field names. Example: `price > 1000 && price < 2000`.
    /// `Option<T>` support could be added if needed (defaulting None to type default).
    /// See [evalexpr] for more information on format, operators, etc. Regex is *not* enabled.
    pub expression: String,
}

/// A message to subscribe to a topic, and optionally a specific filter.
#[derive(Deserialize, Debug, TS)]
#[ts(export)]
pub struct SubscribeRequest {
    /// The topic to subscribe to.  See [step_ingestooor_sdk::schema]
    pub topic: String,
    /// An optional filter to apply to the topic. See [Filter]
    #[ts(optional)]
    pub filter: Option<Filter>,
}

/// A message to unsubscribe from a topic, and optionally a specific filter.
/// There is no way to wildcard unsubscribe from all filters on a topic.
#[derive(Deserialize, Debug, TS)]
#[ts(export)]
pub struct UnsubscribeRequest {
    pub topic: String,
    #[ts(optional)]
    pub filter_id: Option<String>,
}

/// A message with a schema returned to the client
#[derive(Serialize, Debug, TS)]
#[ts(export)]
pub struct SchemaMessage<'a> {
    /// The topic that triggered the message to be sent.
    pub topic: String,
    /// The filter identifier, sent on subscription setup in [Filter], which triggered the message to be sent.
    #[ts(optional)]
    pub filter_id: Option<&'a String>,
    /// The message payload
    pub schema: Schema,
}
