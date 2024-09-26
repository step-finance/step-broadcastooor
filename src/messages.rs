use serde::{Deserialize, Serialize};
use step_ingestooor_sdk::dooot::Dooot;
use ts_rs::TS;

/// A filter allows the server to filter out dooots based on expressions.
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Filter {
    /// An identifier for the filter.  This is used to unsubscribe from a specific filter.  
    /// If two filters have the same filter identifier the behavior is undefined. Don't do that.
    pub id: String,
    /// The expression to filter on.
    /// The dooot's fields of types `i*`, `u*`, `f*`, `String`, `Vec<T>` and `bool` are available in
    /// the expression as their field names. Example: `price > 1000 && price < 2000`.
    /// `Option<T>` unwraps to the value or `()` aka `Empty`.
    /// Deep combinations like `Option<Vec<Option<String>>>` are untested and may not work.
    /// See [evalexpr] for more information on format, operators, etc. Regex is *not* enabled.
    pub expression: String,
}

/// A message to subscribe to a topic, and optionally a specific filter.
#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct SubscribeRequest {
    /// The topic to subscribe to.  See [step_ingestooor_sdk::dooot]
    pub topic: String,
    /// An optional filter to apply to the topic. See [Filter]
    #[ts(optional)]
    pub filter: Option<Filter>,
}

/// A message to unsubscribe from a topic, and optionally a specific filter.
/// There is no way to wildcard unsubscribe from all filters on a topic.
#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct UnsubscribeRequest {
    pub topic: String,
    #[ts(optional)]
    pub filter_id: Option<String>,
}

/// A message with a dooot returned to the client
#[derive(Serialize, Debug, TS)]
#[ts(export)]
pub struct DoootMessage<'a> {
    /// The topic that triggered the message to be sent.
    pub topic: String,
    /// The filter identifier, sent on subscription setup in [Filter], which triggered the message to be sent.
    #[ts(optional)]
    pub filter_id: Option<&'a String>,
    /// The message payload
    pub dooot: Dooot,
}
