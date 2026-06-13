# Task Manager API

A production-structured Rust backend API built with Axum, SQLx, and PostgreSQL. Implements email-based two-factor authentication, role-based access control, task management with assignment, and per-user in-memory caching.

---

## Table of Contents

- [Tech Stack](#tech-stack)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Database Setup](#database-setup)
- [Environment Configuration](#environment-configuration)
- [Running the Server](#running-the-server)
- [API Reference](#api-reference)
- [Full Validation Flow](#full-validation-flow)
- [Final Validation Response](#final-validation-response)
- [Cache Design](#cache-design)
- [Security Design](#security-design)
- [Project Structure](#project-structure)
- [Running Tests](#running-tests)
- [Troubleshooting](#troubleshooting)

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Web Framework | Axum 0.7 |
| Async Runtime | Tokio |
| Database | PostgreSQL 16 |
| ORM / Query | SQLx 0.7 |
| Migrations | SQLx built-in migrator |
| Password Hashing | Argon2 |
| Authentication | JWT (jsonwebtoken 9) |
| 2FA Code Hashing | SHA-256 (sha2 crate) |
| Caching | In-memory DashMap |
| Serialization | Serde + serde_json |
| Logging | tracing + tracing-subscriber |
| Error Handling | thiserror + anyhow |

---

## Architecture

```
src/
├── main.rs              # Entry point, AppState, server boot
├── config/              # Environment config loader
├── db/                  # PostgreSQL connection pool
├── models/              # All entities and request/response DTOs
├── handlers/            # Route handler functions
│   ├── auth.rs          # Login + 2FA verify
│   ├── tasks.rs         # Create, assign, view-my-tasks
│   ├── seed.rs          # Seed users for development
│   └── dev.rs           # Dev-only email log viewer
├── middleware/          # JWT auth middleware
├── routes/              # Axum router wiring
├── services/            # Argon2, JWT, OTP, SHA-256 utilities
├── cache/               # In-memory DashMap cache
└── errors/              # AppError enum + IntoResponse impl
```

---

## Prerequisites

Install the following before proceeding:

### 1. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version   # should print rustc 1.75+
```

### 2. Docker and Docker Compose

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install docker.io docker-compose-plugin -y
sudo systemctl start docker
sudo systemctl enable docker

# Add your user to docker group (avoid sudo)
sudo usermod -aG docker $USER
newgrp docker

# Verify
docker --version
docker compose version
```

### 3. jq (for pretty curl output)

```bash
sudo apt install jq -y
```

### 4. sqlx-cli (optional — migrations auto-run on startup)

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

---

## Installation

```bash
# Clone the repository
git clone https://github.com/your-username/task-manager-api.git
cd task-manager-api

# Install Rust dependencies
cargo build
```

---

## Database Setup

This project uses Docker for PostgreSQL. No local Postgres installation needed.

### Start the database

```bash
docker compose up -d
```

### Verify it is running

```bash
docker compose ps
```

Expected output:
```
NAME              IMAGE         STATUS        PORTS
task_manager_db   postgres:16   Up            0.0.0.0:5433->5432/tcp
```

### Test the connection

```bash
psql -U postgres -h localhost -p 5433 -d task_manager -c "SELECT 1;"
# Password: password
```

Expected output:
```
 ?column?
----------
        1
```

### docker-compose.yml reference

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16
    container_name: task_manager_db
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
      POSTGRES_DB: task_manager
    ports:
      - "5433:5432"
    volumes:
      - task_manager_data:/var/lib/postgresql/data

volumes:
  task_manager_data:
```

> Note: Port `5433` is used on the host to avoid conflicts with any local PostgreSQL on `5432`.

---

## Environment Configuration

```bash
cp .env.example .env
```

`.env` contents:

```env
DATABASE_URL=postgres://postgres:password@localhost:5433/task_manager
JWT_SECRET=super-secret-jwt-key-change-in-production
JWT_EXPIRY_HOURS=24
CHALLENGE_EXPIRY_MINUTES=5
RUST_LOG=debug
```

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | required |
| `JWT_SECRET` | Secret key for signing JWTs | required |
| `JWT_EXPIRY_HOURS` | JWT token lifetime in hours | `24` |
| `CHALLENGE_EXPIRY_MINUTES` | 2FA code expiry in minutes | `5` |
| `RUST_LOG` | Log level filter | `debug` |

---

## Running the Server

```bash
cargo run
```

On first run the server will:
1. Connect to PostgreSQL
2. Auto-run all 4 migrations
3. Start listening on `http://0.0.0.0:8080`

Expected output:
```
>> Starting task-manager-api...
>> DATABASE_URL = postgres://postgres:password@localhost:5433/task_manager
>> Connecting to database...
>> Connected to database!
>> Running migrations...
>> Migrations complete.
>> Server running at http://0.0.0.0:8080
```

---

## API Reference

| Method | Endpoint | Auth Required | Role | Purpose |
|--------|----------|---------------|------|---------|
| POST | `/seed/users` | No | - | Create Admin and James Bond |
| POST | `/auth/login` | No | - | Validate credentials, trigger 2FA |
| GET | `/dev/email-logs/latest` | No | - | View latest verification code (dev only) |
| POST | `/auth/verify-2fa` | No | - | Verify code, receive JWT |
| POST | `/tasks` | Yes | Admin | Create a task |
| POST | `/tasks/assign` | Yes | Admin | Assign tasks to a user |
| GET | `/tasks/view-my-tasks` | Yes | Any | View tasks assigned to me |

---

## Full Validation Flow

Follow these steps exactly to validate the complete workflow.

### Step 1: Seed users

```bash
curl -s -X POST http://localhost:8080/seed/users | jq
```

Expected:
```json
{
  "message": "Users seeded successfully",
  "admin": {
    "email": "admin@example.com",
    "role": "admin"
  },
  "staff": {
    "email": "jamesbond@example.com",
    "role": "staff"
  }
}
```

Credentials created:
- Admin: `admin@example.com` / `Admin@1234`
- James Bond: `jamesbond@example.com` / `Bond@1234`

---

### Step 2: Login as Admin (no JWT returned)

```bash
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "admin@example.com", "password": "Admin@1234"}' | jq
```

Expected:
```json
{
  "login_challenge_id": "<uuid>",
  "message": "Verification code sent to your email. Check /dev/email-logs/latest for the code in development."
}
```

> No JWT is returned here. Save the `login_challenge_id`.

---

### Step 3: Get the verification code

```bash
curl -s http://localhost:8080/dev/email-logs/latest | jq
```

Expected:
```json
{
  "recipient_email": "admin@example.com",
  "subject": "Your 2FA Verification Code",
  "body": "Your verification code is: 845172. It expires in 5 minutes.",
  "sent_at": "2026-06-13T05:00:00Z"
}
```

> Copy the 6-digit code from the `body` field.

---

### Step 4: Verify Admin 2FA and get JWT

```bash
curl -s -X POST http://localhost:8080/auth/verify-2fa \
  -H "Content-Type: application/json" \
  -d '{
    "login_challenge_id": "<challenge_id_from_step_2>",
    "code": "<6_digit_code_from_step_3>"
  }' | jq
```

Expected:
```json
{
  "access_token": "eyJ...",
  "token_type": "Bearer",
  "user": {
    "email": "admin@example.com",
    "role": "admin"
  }
}
```

Save the token:
```bash
ADMIN_TOKEN="eyJ..."
```

---

### Step 5: Create 5 tasks as Admin

```bash
curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Task One", "priority": "high"}' | jq

curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Task Two", "priority": "high"}' | jq

curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Task Three", "priority": "medium"}' | jq

curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Task Four", "priority": "medium"}' | jq

curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Task Five", "priority": "low"}' | jq
```

> Note the `id` values from the first 3 responses for the next step.

---

### Step 6: Assign 3 tasks to James Bond

```bash
curl -s -X POST http://localhost:8080/tasks/assign \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": [
      "<task_one_id>",
      "<task_two_id>",
      "<task_three_id>"
    ],
    "assigned_to_email": "jamesbond@example.com"
  }' | jq
```

Expected:
```json
{
  "assigned_count": 3,
  "assigned_to": "jamesbond@example.com",
  "message": "Successfully assigned 3 task(s) to jamesbond@example.com"
}
```

---

### Step 7: Login as James Bond

```bash
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "jamesbond@example.com", "password": "Bond@1234"}' | jq
```

Save the `login_challenge_id`.

---

### Step 8: Get James Bond's code and verify

```bash
curl -s http://localhost:8080/dev/email-logs/latest | jq

curl -s -X POST http://localhost:8080/auth/verify-2fa \
  -H "Content-Type: application/json" \
  -d '{
    "login_challenge_id": "<challenge_id>",
    "code": "<6_digit_code>"
  }' | jq
```

Save the token:
```bash
BOND_TOKEN="eyJ..."
```

---

### Step 9: James Bond tries to create a task — must return 403

```bash
curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $BOND_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Spy task", "priority": "high"}' | jq
```

Expected:
```json
{
  "error": "Only admin users can create tasks"
}
```

> HTTP status: `403 Forbidden`

---

### Step 10: View James Bond's tasks — cache.hit = false

```bash
curl -s http://localhost:8080/tasks/view-my-tasks \
  -H "Authorization: Bearer $BOND_TOKEN" | jq
```

Expected: 3 tasks, `"cache": { "hit": false }`

---

### Step 11: Call again — cache.hit = true

```bash
curl -s http://localhost:8080/tasks/view-my-tasks \
  -H "Authorization: Bearer $BOND_TOKEN" | jq
```

Expected: same 3 tasks, `"cache": { "hit": true }`

---

## Final Validation Response

First call (`cache.hit = false`):

```json
{
  "user": {
    "email": "jamesbond@example.com",
    "role": "staff"
  },
  "tasks": [
    {
      "id": "1a6069e1-98dd-4fb5-9c30-f32e4e138a46",
      "title": "Task One",
      "status": "todo",
      "priority": "high",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "81b9ed4d-c79a-4f45-a040-da1af751a711",
      "title": "Task Two",
      "status": "todo",
      "priority": "high",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "ef45b9f4-0318-4082-bff5-8cf5912d1ccf",
      "title": "Task Three",
      "status": "todo",
      "priority": "medium",
      "assigned_to": "jamesbond@example.com"
    }
  ],
  "summary": {
    "total_assigned_tasks": 3
  },
  "cache": {
    "hit": false
  }
}
```

Second call (`cache.hit = true`):

```json
{
  "user": {
    "email": "jamesbond@example.com",
    "role": "staff"
  },
  "tasks": [...],
  "summary": {
    "total_assigned_tasks": 3
  },
  "cache": {
    "hit": true
  }
}
```

---

## Cache Design

Redis is not used. An in-memory `DashMap`-based cache is used instead.

| Property | Value |
|----------|-------|
| Cache type | In-memory DashMap |
| Cache key | `tasks:user:{user_id}` |
| TTL | 5 minutes |
| Invalidation | On task assignment or update |
| First request | Loads from DB, `cache.hit = false` |
| Second request | Served from cache, `cache.hit = true` |

**Limitation**: Cache is process-local. In a horizontally scaled deployment, replace DashMap with Redis using the `redis` crate.

---

## Security Design

| Concern | Implementation |
|---------|---------------|
| Password storage | Argon2 hashing (never stored in plain text) |
| 2FA codes | SHA-256 hashed before DB storage |
| Code expiry | 5-minute TTL enforced at verify time |
| Code reuse | `used` flag flipped after first successful verify |
| JWT signing | HMAC-SHA256 with configurable secret |
| Role enforcement | Checked in handler before any DB operation |

---

## Project Structure

```
task-manager-api/
├── Cargo.toml
├── Cargo.lock
├── docker-compose.yml
├── .env.example
├── .gitignore
├── README.md
├── AI_USAGE.md
├── migrations/
│   ├── 20240101000001_create_users.sql
│   ├── 20240101000002_create_tasks.sql
│   ├── 20240101000003_create_login_challenges.sql
│   └── 20240101000004_create_email_logs.sql
└── src/
    ├── main.rs
    ├── config/mod.rs
    ├── db/mod.rs
    ├── errors/mod.rs
    ├── models/mod.rs
    ├── cache/mod.rs
    ├── services/mod.rs
    ├── middleware/mod.rs
    ├── routes/mod.rs
    └── handlers/
        ├── mod.rs
        ├── auth.rs
        ├── tasks.rs
        ├── seed.rs
        └── dev.rs
```

---

## Running Tests

```bash
cargo test
```

---

## Troubleshooting

**Server exits immediately with no output**
- Check your `.env` file exists and has `DATABASE_URL` set correctly
- Run `cargo run 2>&1` to capture stderr

**Cannot connect to database**
- Verify Docker container is running: `docker compose ps`
- Check the port in `DATABASE_URL` matches docker-compose (`5433`)

**`peer authentication failed`**
- Use Docker instead of local Postgres, or fix `pg_hba.conf` to use `md5`

**`Users already seeded` error**
- Reset the database: `docker compose down -v && docker compose up -d`
- Then restart the server and re-seed

**2FA code expired**
- Codes expire in 5 minutes. Run the login step again to get a fresh code.

**Port 5432 already in use**
- Stop local Postgres: `sudo systemctl stop postgresql`
- Or change docker-compose port to `5433:5432` and update `.env`