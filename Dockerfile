# syntax=docker/dockerfile:1

FROM rustlang/rust:nightly-bookworm AS builder
WORKDIR /app

# Pre-fetch dependencies using lockfile (no compile yet)
COPY Cargo.toml Cargo.lock ./
# Create a minimal target so Cargo can parse the manifest and fetch deps
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs && \
    cargo fetch --locked

# Build
COPY . .
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12
WORKDIR /
COPY --from=builder /app/target/release/cncf-people-api /cncf-people-api
# Include assets so /local/people can load /assets/people.json in container
COPY --from=builder /app/assets /assets
USER nonroot:nonroot
EXPOSE 9090
ENV PORT=9090
CMD ["/cncf-people-api"]
