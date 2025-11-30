FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
RUN cargo install sqlx-cli --no-default-features --features postgres
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json

ENV SQLX_OFFLINE true
COPY . .
RUN cargo build --workspace

FROM debian:trixie-slim AS runtime
WORKDIR /app
COPY --from=planner /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder /app/target/debug/api /usr/local/bin/api
COPY --from=builder /app/target/debug/worker /usr/local/bin/worker
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/configuration /configuration
