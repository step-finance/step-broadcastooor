# Step Broadcastooor

## Overview
This application is used to run a socket server and broadcast schema changes over
socketio.

## Client Library
This rust project uses `ts-rs` to generate typescript types for the rust schema message types (from ingestooor). Those types are stored in the sdk folder. These are generated locally by running `cargo test --release`.

The SDK can be built locally by running `npm run build` in `./sdk`.  There is an example use of the sdk in `./sdk/example`.

CI publishes the SDK on `main` branch commits when the ingestooor-sdk version changes (detected in Cargo.lock). **It uses the version from ingestooor-sdk!**  The SDK is published by CI to NPM as `@stepfinance/broadcastooor`.

## Development
Standard rust stuff, just copy `.env.example` to `.env` and update the rabbit url to point to a rabbit server running ingestooor.

### Design
Socketio has a concept of "rooms" that one can join and leave. Ingestooor Data Schemas have a concept of "topics" which are used for subscriptions to high or low level filters. The subscription to data schema topics uses this "rooms" concept in socketio. The words "topic" and "room" are used interchangably occasionally in code.

To make things more confusing, Rabbit also has a concept of "topics" in messaging. Rabbit topics are not used AT ALL in this solution, so do not let that confuse you. 

I *can* see an optimization that could be made whereby a data schema message from ingestooor is published to rabbit using the schema level topic for that message.  This would allow broadcastooor to only subscribe to a subset of messages instead of the whole firehose, and adjust that as socketio clients subscribed to different topics.

### Topics explained

For example, this is the SolTransfer schema class from the Ingestooor SDK crate:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, ToSql, Default, Schema, TS)]
#[postgres(name = "type_time_sol_transfer")]
#[schema(name = "SolTransfer", proc = "indexer.fn_insert_time_sol_transfer")]
pub struct SolTransferSchema {
    pub time: NaiveDateTime,
    /// This field is exposed as a topic
    #[topic]
    pub source_pubkey: String,
    /// This field is exposed as a topic
    #[topic]
    pub destination_pubkey: String,
    pub amount: i64,
    pub parent_program: Option<String>,
}
```
Since the schema attribute does not have a `topic` property inside its struct level declaration, messages not published as topic `SolTransfer`. However, they are published to the more granular topics for the `source_pubkey` and `destination_pubkey` fields. So clients can subscribe to `SolTransfer.source_pubkey.<specific pubkey>`.
Note that if a topic field is `Option<T>`, and is set to `None`, it will put `null` in the last segment of the topic.

### Expressions

Subscriptions can use filters, which allow basic expressions to be used to filter messages server side.  Always subscribe to the most granular topic possible, and then apply additional server side filters using expressions if needed.

These expressions can use any field from the schema as long as it is of type `i64`, `u64`, `f64`, `String`, `bool`. Note that `u64` isn't actually supported for values over `i32::MAX` and will simply be set to value `i32::MAX` for expression evaluation for values greater than that. `Option` types are not supported at all afaik, but support could be added.
