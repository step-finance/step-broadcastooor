FROM rust:1.77-slim-bullseye as build

RUN apt-get update && apt-get install -y pkg-config libssl-dev git openssh-client libclang-dev cmake g++ protobuf-compiler

RUN mkdir -p -m 0600 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts

# Create a new empty shell project
WORKDIR /
RUN cargo new --bin app

# Copy over the manifests
WORKDIR /app
COPY ./.cargo/config.toml ./.cargo/config.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Fetch the dependencies
RUN --mount=type=ssh cargo fetch

# Remove the dummy project
RUN rm -r /app/src

# Copy the source tree
COPY ./src /app/src

# Build for release
RUN cargo build --release --frozen --offline

FROM debian:bullseye-slim
RUN apt update && apt-get install -y ca-certificates
COPY --from=build /app/target/release/step-broadcastooor .
CMD ["./step-broadcastooor"]

