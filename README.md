# Task Manager API

A Rust backend API built with Axum, SQLx, and PostgreSQL implementing authentication, 2FA, role-based access control, task management, and caching.

## Tech Stack

- **Framework**: Axum 0.7
- **Runtime**: Tokio
- **Database**: PostgreSQL + SQLx
- **Auth**: JWT (jsonwebtoken) + Argon2 password hashing
- **2FA**: SHA-256 hashed OTP codes stored in DB, emailed via console/DB log
- **Cache**: In-memory DashMap (documented below)
- **Migrations**: SQLx built-in migrator

## Cache Design

Redis is not used. An in-memory `DashMap`-based cache (`AppCache`) is used instead.

- Per-user cache key: `tasks:user:{user_id}`
- TTL: 5 minutes
- Cache is invalidated when tasks are assigned or updated
- `cache.hit = false` on first DB fetch, `cache.hit = true` on cache hit
- **Limitation**: Cache is process-local. In a multi-instance deployment, use Redis instead.

## Prerequisites

- Rust (stable, edition 2021+)
- PostgreSQL 14+
- `sqlx-cli` for running migrations manually (optional, auto-runs on startup)

## Setup

### 1. Clone and install sqlx-cli

```bash
git clone <repo>
cd task-manager-api
cargo install sqlx-cli --no-default-features --features postgres
```

### 2. Create the database

```bash
createdb task_manager
# or via psql:
psql -U postgres -c "CREATE DATABASE task_manager;"
```

### 3. Configure environment

```bash
cp .env.example .env
# Edit .env with your database credentials
```

`.env` contents:
```
DATABASE_URL=postgres://postgres:password@localhost:5432/task_manager
JWT_SECRET=super-secret-jwt-key-change-in-production
JWT_EXPIRY_HOURS=24
CHALLENGE_EXPIRY_MINUTES=5
RUST_LOG=task_manager_api=debug,tower_http=debug
```

### 4. Run migrations (auto-runs on startup, or manually)

```bash
sqlx migrate run
```

### 5. Start the server

```bash
cargo run
```

Server starts at `http://localhost:8080`

## Validation Flow

Complete this flow end-to-end using curl or Postman.

### Step 1: Seed users

```bash
curl -s -X POST http://localhost:8080/seed/users | jq
```

Creates:
- Admin: `admin@example.com` / `Admin@1234`
- James Bond: `jamesbond@example.com` / `Bond@1234`

### Step 2: Login as Admin (triggers 2FA)

```bash
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "admin@example.com", "password": "Admin@1234"}' | jq
```

Response contains `login_challenge_id`. **No JWT yet.**

### Step 3: Get the verification code

```bash
curl -s http://localhost:8080/dev/email-logs/latest | jq
```

The 6-digit code is in the `body` field.

### Step 4: Verify 2FA and get Admin JWT

```bash
curl -s -X POST http://localhost:8080/auth/verify-2fa \
  -H "Content-Type: application/json" \
  -d '{"login_challenge_id": "<challenge_id>", "code": "<6_digit_code>"}' | jq
```

Save the `access_token` as `ADMIN_TOKEN`.

### Step 5: Create 5 tasks as Admin

```bash
for priority in high high medium medium low; do
  curl -s -X POST http://localhost:8080/tasks \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"title\": \"Task $priority\", \"priority\": \"$priority\"}" | jq
done
```

Note the `id` values from the first 3 responses.

### Step 6: Assign 3 tasks to James Bond

```bash
curl -s -X POST http://localhost:8080/tasks/assign \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["<task_id_1>", "<task_id_2>", "<task_id_3>"],
    "assigned_to_email": "jamesbond@example.com"
  }' | jq
```

### Step 7: Login as James Bond

```bash
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "jamesbond@example.com", "password": "Bond@1234"}' | jq
```

### Step 8: Get James Bond's 2FA code and verify

```bash
curl -s http://localhost:8080/dev/email-logs/latest | jq

curl -s -X POST http://localhost:8080/auth/verify-2fa \
  -H "Content-Type: application/json" \
  -d '{"login_challenge_id": "<challenge_id>", "code": "<6_digit_code>"}' | jq
```

Save token as `BOND_TOKEN`.

### Step 9: James Bond tries to create a task (must fail with 403)

```bash
curl -s -X POST http://localhost:8080/tasks \
  -H "Authorization: Bearer $BOND_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Spy task", "priority": "high"}' | jq
# Expected: 403 Forbidden
```

### Step 10: View James Bond's tasks (cache.hit = false)

```bash
curl -s http://localhost:8080/tasks/view-my-tasks \
  -H "Authorization: Bearer $BOND_TOKEN" | jq
# cache.hit = false
```

### Step 11: View again (cache.hit = true)

```bash
curl -s http://localhost:8080/tasks/view-my-tasks \
  -H "Authorization: Bearer $BOND_TOKEN" | jq
# cache.hit = true
```

## Final Validation Response

```json
{
  "user": {
    "email": "jamesbond@example.com",
    "role": "staff"
  },
  "tasks": [
    {
      "id": "...",
      "title": "Task high",
      "status": "todo",
      "priority": "high",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "...",
      "title": "Task high",
      "status": "todo",
      "priority": "high",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "...",
      "title": "Task medium",
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

Second call returns `"cache": { "hit": true }`.

## API Reference

| Method | Endpoint | Auth | Purpose |
|--------|----------|------|---------|
| POST | `/seed/users` | None | Create Admin and James Bond |
| POST | `/auth/login` | None | Email/password login, triggers 2FA |
| GET | `/dev/email-logs/latest` | None | View latest sent verification code |
| POST | `/auth/verify-2fa` | None | Verify code, receive JWT |
| POST | `/tasks` | Admin JWT | Create a task |
| POST | `/tasks/assign` | Admin JWT | Assign tasks to a user |
| GET | `/tasks/view-my-tasks` | Any JWT | View tasks assigned to me |

## Running Tests

```bash
cargo test
```

## AI Usage

See `AI_USAGE.md`.