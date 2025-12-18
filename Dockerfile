# --- Build Stage ---
FROM rust:alpine3.20 AS builder

ENV PROTOC=/usr/bin/protoc

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    protobuf-dev \
    git

WORKDIR /app

# The .dockerignore will prevent your local 'data/' content from being copied here
COPY . .

# Ensure the directory exists in case the build process looks for it
RUN mkdir -p /app/data

RUN cargo build --release

# --- Runtime Stage ---
FROM alpine:3.20

RUN apk add --no-cache \
    libgcc \
    libssl3 \
    ca-certificates \
    tzdata

WORKDIR /app

# Create an empty data directory for the container environment
RUN mkdir -p /app/data

COPY --from=builder /app/target/release/rosetta /app/rosetta

# Volume remains empty until the user mounts their own data
VOLUME /app/data

CMD ["./rosetta"]
