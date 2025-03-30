# ./new-york/Dockerfile
FROM rust:latest AS builder

# Set the working directory
WORKDIR /usr/src/app

# Install build dependencies, including clang and libclang-dev for bindgen and OpenSSL
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev clang libclang-dev && \
    rm -rf /var/lib/apt/lists/*

# Install libssl3 for the build environment (if needed)
RUN apt-get update && apt-get install -y libssl3 && apt-get clean

# Copy the entire workspace into the container
COPY ../../ .

# Navigate to the specific crate directory and build
WORKDIR /usr/src/app/new-york-light
RUN cargo build --release

# Final runtime image
FROM debian:bookworm-slim

# Install runtime dependencies: libssl3 and ca-certificates (for proper OpenSSL functionality)
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    apt-get clean

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/guest /usr/local/bin/guest
COPY --from=builder /usr/src/app/target/release/host /usr/local/bin/host

# Set the entrypoint
CMD ["guest"]
