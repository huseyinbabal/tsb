# Stage 1: Build
FROM rust:latest AS builder

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src and resources to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
COPY resources ./resources

# Build dependencies only
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime
FROM debian:trixie-slim

# Install CA certificates for TLS
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/tspring /usr/local/bin/tspring

# Set terminal for TUI
ENV TERM=xterm-256color

ENTRYPOINT ["tspring"]
