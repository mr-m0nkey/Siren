# Update to a specific version
FROM rust:latest AS builder 


WORKDIR /usr/src/siren


# Copy only Cargo.toml and Cargo.lock first to cache dependencies
COPY Cargo.toml Cargo.lock ./

# Copy only the main source file for now
COPY src/main.rs src/main.rs

RUN cargo build --release

COPY . .



# TODO: Add volumes for config files
FROM debian:trixie-slim

# Install OpenSSL 3 runtime and CA certificates
RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

ENV TELOXIDE_TOKEN=
ENV CHAT_ID=

COPY --from=builder /usr/src/siren/target/release/siren .
COPY --from=builder /usr/src/siren/config /usr/local/bin/config


CMD ["./siren"]
