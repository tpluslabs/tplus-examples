FROM rust:latest AS builder

WORKDIR /usr/src/app

# Install build dependencies, including clang, libclang-dev, and OpenSSL headers/libraries
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev clang libclang-dev && \
    rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y libssl3 && apt-get clean

# Copy the entire workspace into the container
COPY . .

WORKDIR /usr/src/app/surpluser

# Set environment variables for OpenSSL linking during build
ENV OPENSSL_DIR=/usr
ENV OPENSSL_STATIC=1

RUN cargo build --release

# Final runtime image
FROM debian:bookworm-slim

# Install runtime dependencies: libssl3 for OpenSSL and ca-certificates for SSL certificate verification.
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    apt-get clean

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/surpluser /usr/local/bin/surpluser

# Set the entrypoint
CMD ["surpluser"]
