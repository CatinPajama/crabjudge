# CrabJudge

CrabJudge is a simple online judge prototype written in Rust. It consists of an HTTP API, a worker that executes submitted code inside Docker containers, and shared models + configuration utilities.

This README covers repository layout, architecture, setup, system design, and API endpoints with links to key source files and symbols.

## Overview / System design

Components:
- API: receives signup/login and submit requests and enqueues tasks into RabbitMQ. 
- Worker: listens on RabbitMQ exchanges (one per runtime) and runs incoming tasks in container pool.
- Container manager / pool: creates Docker images and reuses running containers. 
- Execution: actual code run uses Docker exec attaching STDIN/STDOUT.
- Configuration loader: typed config based on YAML + env.

Message flow (high level):
1. Client calls POST /{problemID}/submit (API) → handler 
2. API validates session + environment and publishes a JSON  to the exchange named after the runtime (example: `python:3.12`).
3. Worker (one per runtime config) consumes messages from its queue, uses a Container Pool to obtain a container, and runs the submission 
4. Worker compares output and writes a row into `submit_status` table.


## Configuration

Configs are loaded from `configuration/<env>/` (default env `local`). To override, use environment variables with prefix `CRABJUDGE_<configname>_<key>`

## Setup (local development)

Prerequisite:
- Docker and docker daemon running (worker uses host Docker socket).
- Rust toolchain (stable) and cargo.
- sqlx-cli for database migrations (installed in Dockerfile during build and used by scripts).
- Docker Compose (optional) for running full stack.

Quick start with docker-compose (recommended for integration):
1. Start services:
   - docker-compose up --build
   - This brings up `postgres`, `redis`, `rabbitmq`, `api`, and `worker` as configured in [docker-compose.yaml](docker-compose.yaml).
2. API's container runs migrations at startup (see compose command).

Local (dev) run (without containers):
1. Start Postgres / Redis / RabbitMQ locally or via docker.
2. Ensure DATABASE_URL and SQLX_OFFLINE (optional) are set. Example `.env.sample` provided.
3. Run DB migrations:
   - Install sqlx-cli (or use Docker image with it).
   - Run `sqlx database create` and `sqlx migrate run` (migrations folder is [migrations/](migrations/)).
4. Build and run:
   - API: `cargo run -p api`
   - Worker: `cargo run -p worker`
   - Both use [`models::Settings`](models/src/lib.rs) / config loader.

<!-- Run tests:
- API integration tests spawn ephemeral DBs and run migrations. See [api/tests/utils.rs](api/tests/utils.rs).
- Worker unit/integration test: [worker/tests/verify_output.rs](worker/tests/verify_output.rs) uses Docker to create a Python container and run code.

Useful commands:
- Build workspace: cargo build --workspace
- Run API: cargo run -p api
- Run Worker: cargo run -p worker
- Run tests: cargo test --workspace -->

---

## API Endpoints


1. POST /signup
   - Body: form fields `username`, `password`
   - Response: 200 OK on success, 400 on invalid, 500 on DB error
   - Creates a user with Argon2 hashed password.

2. POST /login
   - Authentication: Basic auth header (username:password)
   - Response:
     - 200 OK on success (session "user_id" inserted)
     - 401 Unauthorized on bad credentials
     - 400 Bad Request with cookie `login_error` on invalid input

3. POST /{problemID}/submit
   - Auth: requires session (user logged in)
   - Body (JSON):
     - { "code": "<source>", "env": "<runtime_key>" }
   - Behavior:
     - Validates env against loaded config.
     - Publishes a Task to the RabbitMQ exchange named by env.
   - Response: 200 OK on enqueue, 400 if invalid env, 401 if not logged in

4. GET /{submissionID}/status
   - Auth: requires session
   - Response: JSON with { user_id, problem_id, status, output }

5. GET /{problemID}/submissions
   - Auth: requires session
   - Response: list of submission ids for the user/problem

---


## Extending / adding runtimes

1. Add runtime config in [configuration/local/runtimeconfigs.yaml](configuration/local/runtimeconfigs.yaml).
2. Ensure the image is pullable (Docker Hub or private).
3. The API will publish to the exchange named by the runtime key; Worker spawn per runtime and listens for messages on the queue/exchange pair.

---

## Migrations & DB

Migrations live in [migrations/](migrations/). They define `users`, `problems`, `problem_testcases`, and `submit_status`. The API tests and the docker-compose setup run migrations automatically.
