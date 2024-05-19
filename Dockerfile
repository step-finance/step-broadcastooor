FROM rust:1.76-slim-bullseye as build

RUN apt-get update && apt-get install -y pkg-config libssl-dev

# Create a new empty shell project
WORKDIR /
RUN cargo new --bin app

# Copy over the manifests
WORKDIR /app
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Build the dependencies
RUN cargo build --release

# Remove the dummy project
RUN rm -r /app/src
RUN rm -r /app/target/release/temp

# Copy the source tree
COPY ./src /app/src

# Build for release
RUN CARGO_NET_GIT_FETCH_WITH_CLI=true && cargo build --release

FROM debian:bullseye-slim
RUN apt update && apt-get install -y ca-certificates
COPY --from=build /app/target/release/step-broadcastooor .
CMD ["./step-broadcastooor"]
