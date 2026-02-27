# CrabJudge

[![Rust](https://img.shields.io/badge/Rust-1.9x+-black?logo=rust)](https://www.rust-lang.org/)
[![Actix Web](https://img.shields.io/badge/Actix-Web-blue)](https://actix.rs/)
[![Next.js](https://img.shields.io/badge/Next.js-Frontend-black?logo=next.js)](https://nextjs.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-Database-316192?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Redis](https://img.shields.io/badge/Redis-Session%20%26%20Rate%20Limit-red?logo=redis&logoColor=white)](https://redis.io/)
[![RabbitMQ](https://img.shields.io/badge/RabbitMQ-Queue-orange?logo=rabbitmq&logoColor=white)](https://www.rabbitmq.com/)
[![Docker](https://img.shields.io/badge/Docker-Containers-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CrabJudge is a Rust-based online judge platform with a Next.js frontend. It supports authenticated submissions, asynchronous judging via workers, runtime-based code execution inside isolated Docker containers, and real-time result polling.

https://github.com/user-attachments/assets/67a08b55-11da-404b-8241-36216fc86b3e

---

## Features

- **Email-verified signup** with token-based confirmation flow
- **Session-based authentication** backed by Redis
- **Problem listing & retrieval** with difficulty levels
- **Multi-language code submission** — runtime environment is configurable (Python, C++, JavaScript, etc.)
- **Live status polling** — clients poll submission status until judging completes
- **Per-user stats** — solved problems grouped by difficulty, rendered as charts
- **Role-based access** — `User`, `ProblemSetter`, and `Admin` roles with hierarchical permissions
- **Rate limiting** — per-user API rate limits powered by Redis

---

## Architecture

CrabJudge follows a decoupled **API + Worker** design connected by a message queue.

### Components

| Component | Role |
|---|---|
| **API** (Actix Web) | Handles auth, sessions, problem/submission endpoints, and publishes tasks to RabbitMQ |
| **Worker** (Tokio + Bollard) | Consumes queue messages, executes user code in isolated Docker containers, writes results back to the database |
| **PostgreSQL** | Persistent storage for users, problems, testcases, and submission statuses |
| **Redis** | Session store and request rate-limiting backend |
| **RabbitMQ** | Asynchronous transport between the API and worker, with dead-letter routing for failed tasks |
| **Frontend** (Next.js) | Server-rendered problem pages, client-side code editor (Monaco), submission UI with live polling |

### Submission Flow

1. Client sends `POST /{problemID}/submit` with code and target runtime
2. API validates the session and runtime environment, inserts a `PENDING` submission record into PostgreSQL
3. API publishes a `WorkerTask` to the RabbitMQ **`code`** exchange, routed by runtime key (e.g. `python:3.12`, `gcc`)
4. Worker consumes the message, acquires a pooled Docker container, copies the code in, and executes it with the problem's testcase as stdin
5. Worker compares output against the expected result (whitespace-normalized), then writes the final status (`PASSED`, `WRONG ANSWER`, `TLE`, `MLE`, `SEGFAULT`) and output back to PostgreSQL
6. Client polls `GET /{submissionID}/status` until the status is no longer `PENDING`

### Frontend ↔ Backend

The Next.js frontend proxies API requests through its own route handlers under `app/api/`. This keeps the backend URL server-side only and forwards session cookies transparently.

---

## Redis Usage

Redis serves two purposes:

- **Session storage** — `SessionAuth` payloads (user ID + role) are stored in Redis-backed sessions via `actix-session`. This keeps authentication checks fast and allows the API to remain stateless across restarts.
- **Rate limiting** — `actix-limitation` uses Redis to enforce per-user request quotas, keyed by the authenticated user's ID.

---

## RabbitMQ Queues & Dead Lettering

- The API publishes submissions to exchange **`code`** with the routing key set to the runtime environment name (e.g. `python:3.12`, `gcc`).
- Each worker declares and binds its own runtime queue, then consumes from it.
- All runtime queues are configured with dead-letter routing:
  - **Dead-letter exchange:** `dlx`
  - **Dead-letter queue:** `dlq`
- If a worker fails to process a message (container crash, DB error, etc.), the message is nacked without requeue and routed to `dlq` for inspection or retry — no tasks are silently lost.

---

## Error Handling & Fault Tolerance

- **API** uses structured `ResponseError` implementations per route, mapping validation errors to `400`, DB/queue failures to `500`, and auth failures to `401`/`403`.
- **Worker** uses a typed `ExecError` enum covering database, Docker, queue, and pool failures.
- Testcase fetching uses **exponential backoff** to handle transient database connectivity issues.
- **Graceful shutdown** — the worker listens for `SIGTERM` and `CTRL-C`, cancels in-flight tasks via a `CancellationToken`, waits for the `TaskTracker` to drain, and tears down the container pool cleanly.
- **Container isolation** — each submission runs in a Docker container with `--network=none`, a hard memory limit + swap cap, a PID limit of 16, and `no-new-privileges` security option. A configurable timeout kills long-running processes.
- **Output comparison** normalizes trailing whitespace and newlines to reduce false-negative wrong-answer verdicts.

---

## API Overview

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/signup` | Register with email, username, password |
| `POST` | `/signup/confirmation` | Verify email with token |
| `POST` | `/login` | Authenticate (HTTP Basic Auth) |
| `GET` | `/problems` | List all problems (paginated) |
| `GET` | `/problem/{problemID}` | Get a single problem |
| `POST` | `/{problemID}/submit` | Submit code for judging |
| `GET` | `/{submissionID}/status` | Poll submission status |
| `GET` | `/{problemID}/submissions` | List user's submissions for a problem |
| `GET` | `/stats` | Get user's solve stats by difficulty |
| `POST` | `/createProblem` | Create a problem (ProblemSetter+ role required) |

---

## Setup

### Prerequisites

- **Rust** (stable) + Cargo
- **Docker** + Docker daemon running
- **Node.js** 18+ and npm
- **sqlx-cli**:
  ```bash
  cargo install sqlx-cli --no-default-features --features postgres
  ```

### Option A: Docker Compose (recommended for local dev)

Spin up PostgreSQL, Redis, and RabbitMQ with the dev compose file, then run the API, worker, and frontend locally:

```bash
# Start infrastructure services
docker compose -f docker-compose.dev.yaml up -d

# Load environment
cp .env.sample .env   # edit values as needed
set -a && source .env && set +a

# Run migrations
sqlx database create
sqlx migrate run

# Start backend (in separate terminals)
cargo run -p api
cargo run -p worker

# Start frontend
cd crabjudge_frotend
npm install
npm run dev
```

### Option B: Full Docker Compose (production-like)

```bash
docker compose -f docker-compose.yaml up --build
```

### Option C: Manual dependency setup

Use the provided scripts if you prefer to manage containers yourself:

```bash
./scripts/init_db.sh      # PostgreSQL
./scripts/init_redis.sh    # Redis
# Start RabbitMQ separately
```

Then follow the same steps as Option A from "Load environment" onward.

---

## Configuration & Environment Variables

Configuration is loaded from `configuration/local.yaml` and can be overridden with environment variables prefixed with `CRABJUDGE_` (double underscore `__` for nesting).

| Variable | Description | Default |
|---|---|---|
| `CRABJUDGE_APPLICATION__HOST` | API bind host | `127.0.0.1` |
| `CRABJUDGE_APPLICATION__PORT` | API bind port | `8080` |
| `CRABJUDGE_APPLICATION__BASE_URL` | Public API URL | `http://127.0.0.1:8080` |
| `CRABJUDGE_DATABASE__USER` | PostgreSQL user | `api` |
| `CRABJUDGE_DATABASE__PASSWORD` | PostgreSQL password | `123` |
| `CRABJUDGE_DATABASE__HOST` | PostgreSQL host | `localhost` |
| `CRABJUDGE_DATABASE__PORT` | PostgreSQL port | `5432` |
| `CRABJUDGE_DATABASE__DBNAME` | PostgreSQL database name | `judge` |
| `CRABJUDGE_REDIS__HOST` | Redis host | `localhost` |
| `CRABJUDGE_REDIS__PORT` | Redis port | `6379` |
| `CRABJUDGE_RABBITMQ__HOST` | RabbitMQ host | `localhost` |
| `CRABJUDGE_RABBITMQ__PORT` | RabbitMQ port | `5672` |
| `CRABJUDGE_RABBITMQ__VHOST` | RabbitMQ virtual host | `/` |
| `CRABJUDGE_EMAIL_CLIENT__BASE_URL` | Email provider endpoint | — |
| `CRABJUDGE_EMAIL_CLIENT__SENDER_EMAIL` | Verified sender address | — |
| `CRABJUDGE_EMAIL_CLIENT__AUTHORIZATION_TOKEN` | Email API token | — |
| `BACKEND_URL` | Backend URL for frontend proxy | `http://localhost:8080` |

Runtime configs (languages, memory limits, timeouts, Docker images) are defined in the YAML config under `runtimeconfigs`.

---

## License

This project is open source under the [MIT License](LICENSE).