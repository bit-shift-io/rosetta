# Build Stage
FROM rust:alpine3.20 AS builder

# Install build dependencies
# - musl-dev: Standard C library headers
# - openssl-dev: Required for encryption
# - protobuf-dev: Required for compiling WhatsApp protocol buffers
# - git: Required for fetching git dependencies
RUN apk add --no-cache musl-dev openssl-dev protobuf-dev git

WORKDIR /app
COPY . .

# Build the release binary
RUN cargo build --release

# Runtime Stage
FROM alpine:3.20

# Install runtime dependencies
# - libgcc: Runtime library for GCC
# - libssl3: Runtime OpenSSL library
# - ca-certificates: Required for HTTPS requests
RUN apk add --no-cache libgcc libssl3 ca-certificates

WORKDIR /app
COPY --from=builder /app/target/release/rosetta /app/rosetta

# Default command
CMD ["./rosetta"]
