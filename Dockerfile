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




FROM chef AS builder-dev
#ENV SQLX_OFFLINE true
RUN cargo install cargo-watch
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .


FROM debian:trixie-slim AS runtime-prod
WORKDIR /app
COPY --from=planner /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder-prod /app/target/release/api /usr/local/bin/api
COPY --from=builder-prod /app/target/release/worker /usr/local/bin/worker
COPY --from=builder-prod /app/migrations /app/migrations
COPY --from=builder-prod /app/configuration /configuration

FROM rust:latest AS runtime-dev
WORKDIR /app
COPY --from=builder-dev /usr/local/cargo/bin/cargo-watch /usr/local/bin/cargo-watch
COPY --from=planner /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder-dev /app/migrations /app/migrations
COPY --from=builder-dev /app/configuration /configuration

