# Step Broadcastooor

## Overview
This application is used to run a socket server and broadcast schema changes over
socketio.

## Development
Standard rust stuff, just copy `.env.example` to `.env` and update the rabbit url to point to a rabbit server
running ingestooor.

## Client Library
This rust project uses `ts-rs` to generate typescript types for the rust schema message types (from ingestooor). Those types are stored in the sdk folder. These are generated locally by running `cargo test --release`.

The SDK can be built locally by running `npm run build` in `./sdk`.  There is an example use of the sdk in `./sdk/example`.

The SDK is published by CI to NPM as `@stepfinance/broadcastooor`.