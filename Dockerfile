# --- Build Stage ---
FROM rust:alpine3.20 AS builder

ENV PROTOC=/usr/bin/protoc
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include/openssl

# 1. ADDED 'sqlite-dev' here so the linker can find SQLite headers
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconfig \
    perl \
    protobuf-dev \
    gcc \
    git \
    sqlite-dev

WORKDIR /app
COPY . .

# 2. MODIFIED the build command to include RUSTFLAGS
# This disables 'crt-static' which prevents the segfault on Alpine
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    RUSTFLAGS="-C target-feature=-crt-static" cargo build --release && \
    cp target/release/rosetta /app/rosetta-bin

# --- Runtime Stage ---
FROM alpine:3.20

# 3. ADDED 'sqlite-libs' and 'libstdc++' so the binary can find them at runtime
RUN apk add --no-cache \
    libgcc \
    libssl3 \
    ca-certificates \
    tzdata \
    bash \
    curl \
    sqlite-libs \
    libstdc++

WORKDIR /app
RUN mkdir -p /app/data
COPY --from=builder /app/rosetta-bin /app/rosetta

VOLUME /app/data
CMD ["./rosetta"]
