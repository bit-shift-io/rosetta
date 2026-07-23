# --- Build Stage ---
FROM docker.io/library/rust:alpine3.24 AS builder

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

# 1. Copy manifests first to cache dependency downloads/builds
COPY Cargo.toml Cargo.lock ./

# 2. Build dependencies only (dummy source)
RUN mkdir src && \
    echo "pub fn main() {}" > src/lib.rs && \
    echo "fn main() {}" > src/main.rs && \
    RUSTFLAGS="-C target-feature=-crt-static" cargo build --release && \
    rm -rf src

# 3. Copy real source and final build
COPY src ./src
COPY data ./data

# Touch both main.rs and lib.rs to ensure both binary and library are rebuilt with real source
RUN touch src/main.rs src/lib.rs && \
    RUSTFLAGS="-C target-feature=-crt-static" cargo build --release && \
    cp target/release/rosetta /app/rosetta-bin

# --- Runtime Stage ---
FROM alpine:3.24

# 3. ADDED 'sqlite-libs' and 'libstdc++' so the binary can find them at runtime
RUN apk add --no-cache \
    libssl3 \
    ca-certificates \
    tzdata \
    sqlite-libs \
    libstdc++ \
    libgcc
    #bash \
    #curl \


WORKDIR /app
RUN mkdir -p /app/data
COPY --from=builder /app/rosetta-bin /app/rosetta

VOLUME /app/data
CMD ["./rosetta"]
