FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p gents-harness --locked
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/gents-harness /usr/local/bin/gents-harness
COPY web ./web
COPY config ./config
ENV BIND=0.0.0.0:8787 AUTH_MODE=entra CONFIG=config/local.json
RUN mkdir -p /app/data && chown 65532:65532 /app/data
USER 65532:65532
EXPOSE 8787
CMD ["gents-harness"]
