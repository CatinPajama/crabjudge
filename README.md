# CrabJudge

CrabJudge is an online judge prototype written in Rust. It consists of an HTTP API that handles user authentication and code submissions, a worker that executes submitted code inside Docker containers with resource limits, and shared models + configuration utilities.

---
## Overview / System Design

### Components

1. **API** (Actix-web)
   - Handles signup/login with Argon2 password hashing
   - Session management via Redis
   - Receives code submissions and publishes tasks to RabbitMQ
   - Routes: `/signup`, `/login`, `/{problemID}/submit`, `/{submissionID}/status`, etc.

2. **Worker** (Tokio async runtime)
   - Spawns one listener per runtime (e.g., python:3.12, node:20, gcc)
   - Consumes messages from RabbitMQ exchanges
   - Uses a deadpool-based container pool to reuse Docker containers
   - Executes code with STDIN/STDOUT and enforces timeouts/memory limits
   - Writes results to `submit_status` table

3. **Container Manager / Pool**
   - Creates Docker containers from images on startup (once per runtime)
   - Reuses containers via deadpool's `Manager` trait
   - Tracks created container IDs and performs best-effort cleanup on shutdown
   - Supports configurable memory limits and timeout per runtime

4. **Docker Execution**
   - Uses Docker API (via bollard crate) to exec commands inside containers
   - Pipes user code to stdin, captures stdout/stderr
   - Records exit codes (137=OOM, 139=SIGSEGV, 124=timeout, etc.)

5. **Configuration Loader**
   - Loads YAML configs from `configuration/<env>.yaml` (default: `local`)

### Message Flow

```
1. Client calls POST /{problemID}/submit (API)
   ↓
2. API validates session & environment, creates submit_status row
   ↓
3. API publishes WorkerTask JSON to RabbitMQ exchange named by runtime key (e.g., "python:3.12")
   ↓
4. Worker consumes message from queue, obtains container from pool
   ↓
5. Worker fetches testcase from DB, executes code in container with timeout/memory limits
   ↓
6. Worker compares output, writes status + output to submit_status row
   ↓
7. Client queries GET /{submissionID}/status to retrieve result
```


## Configuration

Config is loaded from `configuration/<env>.yaml` (default env is `local`) or via environment variables. Files are YAML-based:


- appliation — API host & port
- database — PostgreSQL connection details
- redis — Redis host & port (for sessions)
- rabbitmq — RabbitMQ connection details
- runtimeconfigs — Per-runtime configs (image, compile cmd, run cmd, timeout, memory)


See [configuration/local.yaml](configuration/local.yaml) for examples.

---

## Setup (Local Development)

### Prerequisites

- Docker and Docker daemon running (worker uses host socket at `/var/run/docker.sock`)
- Rust toolchain (stable) and cargo
- sqlx-cli (installed in Dockerfile, can be installed locally: `cargo install sqlx-cli --no-default-features --features postgres`)
- Docker Compose (optional, for full-stack runs)

### Quick Start with Docker Compose (Recommended)

```bash
docker-compose up --build
```

This brings up:
- PostgreSQL (port 5432)
- Redis (port 6379)
- RabbitMQ (port 5672 + management UI on 15672)
- API (port 8080)
- Worker (listens on RabbitMQ, uses host Docker socket)

The API container runs migrations automatically at startup.

### Development

1. Start services locally or via docker:
   ```bash
      docker compose -f docker-compose.dev.yaml up
   ```

2. Set environment variables:
   Rename .env.sample to .env and set email env variables
   ```bash
      set -a
      source .env
      set +a
   ```

3. Run migrations:
   ```bash
   sqlx database create
   sqlx migrate run
   ```

4. Run API and Worker in separate terminals:
   ```bash
   cargo run -p api
   cargo run -p worker
   ```

---

## API Endpoints

### POST /signup
- **Body:** Form fields `username`, `password`, `email`
- **Response:** 200 OK on success, 400 on invalid input, 500 on DB/email error
- **Behavior:** Creates a user with Argon2-hashed password, role defaults to "user"

### POST /login
- **Auth:** Basic auth header (base64(username:password))
- **Response:**
  - 200 OK on success (inserts session into Redis)
  - 401 Unauthorized on bad credentials
  - 400 Bad Request on invalid input
- **Behavior:** Session contains user_id and role

### POST /{problemID}/submit
- **Auth:** Requires session (user logged in)
- **Body (JSON):** `{ "code": "<source_code>", "env": "<runtime_key>" }`
- **Response:** 
  - 200 OK with `{ "submission_id": <id> }` on success
  - 400 if invalid env, 401 if not logged in, 500 on DB/queue error
- **Behavior:**
  - Validates `env` against loaded runtime configs
  - Creates `submit_status` row with status='PENDING'
  - Publishes [`WorkerTask`](models/src/lib.rs) JSON to RabbitMQ exchange named by `env`

### GET /{submissionID}/status
- **Auth:** Requires session
- **Response:** JSON with `{ user_id, problem_id, status, output }`
- **Behavior:** Fetches latest submission status from DB

### GET /{problemID}/submissions
- **Auth:** Requires session
- **Response:** List of submission IDs for the current user/problem
- **Query:** Filters by user_id and problem_id

### GET /problem/{problemID}
- **Auth:** None
- **Response:** JSON with `{ problem_id, title, difficulty, statement }`

### GET /problems
- **Auth:** None
- **Query params:** `?limit=<n>&offset=<m>` (default limit=50)
- **Response:** Paginated list of problems

### GET /stats
- **Auth:** Requires session
- **Response:** User's passing stats by difficulty (count of distinct problems solved)

### POST /createProblem
- **Auth:** Requires session + ProblemSetter or Admin role
- **Body:** Form fields `title`, `difficulty`, `statement`, `testcase`, `output`
- **Response:** 200 OK on success, 401 if unauthorized, 500 on error

---

## Runtime Configuration

Add runtimes in [configuration/local/runtimeconfigs.yaml](configuration/local/runtimeconfigs.yaml):

```yaml
runtimeconfigs:
  "python:3.12":
    image: python:3.12-slim
    run: python /tmp/file
    compile: null           # optional
    timeout: 2              # seconds
    memory: 67108864        # bytes (64 MB)
  
  "gcc":
    image: frolvlad/alpine-gcc
    compile: gcc -x c /tmp/file -o /tmp/a.out
    run: /tmp/a.out
    timeout: 2
    memory: 67108864
```

The key (e.g., "python:3.12") is the exchange name; the image is pulled by the worker on startup
Ensure the Docker image is pullable (public or in your registry).
The API will auto-publish to exchange "ruby:3.2"; worker will spawn a listener and process tasks.

---

## Design Notes & Improvements

### Current Strengths
- Clean separation: API enqueues, Worker executes, DB persists
- Async/concurrent execution via Tokio
- Resource limits enforced at container level (memory, timeout)
- Session management via Redis
- Graceful shutdown with container cleanup
- Composable runtime configs (easy to add languages)

### Known Limitations & TODOs

1. **Error Handling**: Some `.unwrap()` calls in task handlers could leak resources on panic. Should use proper error propagation with error tracking for observability.

2. **Queue Acking**: Tasks are acked after completion (with potential loss on worker crash). Consider acking only after successful DB insert.

3. **Image Cleanup**: Pulled images are not removed on shutdown. Consider adding a prune mechanism for long-running deployments.


4. **Logging & Observability**: Add structured logging (tracing/tokio-console) for better debugging and performance profiling.

---

