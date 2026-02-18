FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
RUN cargo install sqlx-cli --no-default-features --features postgres
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder-prod
ENV SQLX_OFFLINE true
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --workspace --release


FROM debian:trixie-slim AS runtime-prod
WORKDIR /app
COPY --from=planner /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder-prod /app/target/release/api /usr/local/bin/api
COPY --from=builder-prod /app/target/release/worker /usr/local/bin/worker
COPY --from=builder-prod /app/migrations /app/migrations
COPY --from=builder-prod /app/configuration /app/configuration