//! Integration tests for the Task Manager API.
//!
//! Requires a running PostgreSQL database configured via DATABASE_URL in .env.
//!
//! Run with:
//!   cargo test -- --test-threads=1 --nocapture

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use task_manager_api::{build_app, cache::AppCache, config::Config, db, AppState};
use tower::ServiceExt;

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn setup() -> (axum::Router, sqlx::PgPool) {
    dotenvy::dotenv().ok();
    let config = Config::from_env().expect("Failed to load config");
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to connect to test database — is DATABASE_URL set and Postgres running?");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(config),
        cache: AppCache::new(),
    };
    (build_app(state), pool)
}

async fn clean_db(pool: &sqlx::PgPool) {
    sqlx::query(
        "TRUNCATE TABLE email_logs, login_challenges, tasks, users RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("Failed to truncate test tables");
}

async fn response_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn post(app: &axum::Router, uri: &str, payload: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, response_json(res.into_body()).await)
}

async fn post_auth(
    app: &axum::Router,
    uri: &str,
    payload: Value,
    token: &str,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, response_json(res.into_body()).await)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, response_json(res.into_body()).await)
}

async fn get_auth(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, response_json(res.into_body()).await)
}

/// Complete the full login → OTP → verify flow and return a JWT.
async fn get_token(app: &axum::Router, email: &str, password: &str) -> String {
    let (status, body) = post(
        app,
        "/auth/login",
        json!({ "email": email, "password": password }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {body}");
    let challenge_id = body["login_challenge_id"].as_str().unwrap().to_string();

    let (status, email_log) = get(app, "/dev/email-logs/latest").await;
    assert_eq!(status, StatusCode::OK, "email log fetch failed: {email_log}");

    // Email body: "Your verification code is: XXXXXX. It expires in N minutes."
    let email_body = email_log["body"].as_str().unwrap();
    let otp = email_body
        .split("Your verification code is: ")
        .nth(1)
        .unwrap()
        .split('.')
        .next()
        .unwrap()
        .trim()
        .to_string();

    let (status, auth_body) = post(
        app,
        "/auth/verify-2fa",
        json!({ "login_challenge_id": challenge_id, "code": otp }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "verify-2fa failed: {auth_body}");
    auth_body["access_token"].as_str().unwrap().to_string()
}

// ── Seed tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_seed_users_success() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;

    let (status, body) = post(&app, "/seed/users", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Users seeded successfully");
    assert_eq!(body["admin"]["email"], "admin@example.com");
    assert_eq!(body["admin"]["role"], "admin");
    assert_eq!(body["staff"]["email"], "jamesbond@example.com");
    assert_eq!(body["staff"]["role"], "staff");
}

#[tokio::test]
async fn test_seed_users_already_seeded() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;

    post(&app, "/seed/users", json!({})).await;
    let (status, body) = post(&app, "/seed/users", json!({})).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("already seeded"));
}

// ── Auth tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_login_user_not_found() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;

    let (status, body) = post(
        &app,
        "/auth/login",
        json!({ "email": "ghost@example.com", "password": "irrelevant" }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Invalid email or password"));
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let (status, body) = post(
        &app,
        "/auth/login",
        json!({ "email": "admin@example.com", "password": "wrong" }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Invalid email or password"));
}

#[tokio::test]
async fn test_login_success() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let (status, body) = post(
        &app,
        "/auth/login",
        json!({ "email": "admin@example.com", "password": "Admin@1234" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["login_challenge_id"].is_string());
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Verification code"));
}

#[tokio::test]
async fn test_verify_2fa_invalid_challenge_id() {
    let (app, _pool) = setup().await;

    let (status, body) = post(
        &app,
        "/auth/verify-2fa",
        json!({
            "login_challenge_id": "00000000-0000-0000-0000-000000000000",
            "code": "123456"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("Invalid challenge ID"));
}

#[tokio::test]
async fn test_verify_2fa_invalid_code() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let (_, login_body) = post(
        &app,
        "/auth/login",
        json!({ "email": "admin@example.com", "password": "Admin@1234" }),
    )
    .await;
    let challenge_id = login_body["login_challenge_id"].as_str().unwrap();

    let (status, body) = post(
        &app,
        "/auth/verify-2fa",
        json!({ "login_challenge_id": challenge_id, "code": "000000" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Invalid verification code"));
}

#[tokio::test]
async fn test_verify_2fa_code_already_used() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let (_, login_body) = post(
        &app,
        "/auth/login",
        json!({ "email": "admin@example.com", "password": "Admin@1234" }),
    )
    .await;
    let challenge_id = login_body["login_challenge_id"].as_str().unwrap();

    let (_, email_log) = get(&app, "/dev/email-logs/latest").await;
    let otp = email_log["body"]
        .as_str()
        .unwrap()
        .split("Your verification code is: ")
        .nth(1)
        .unwrap()
        .split('.')
        .next()
        .unwrap()
        .trim()
        .to_string();

    // First use — succeeds
    let (status, _) = post(
        &app,
        "/auth/verify-2fa",
        json!({ "login_challenge_id": challenge_id, "code": otp }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Second use — must fail
    let (status, body) = post(
        &app,
        "/auth/verify-2fa",
        json!({ "login_challenge_id": challenge_id, "code": otp }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("already been used"));
}

#[tokio::test]
async fn test_full_auth_flow() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let token = get_token(&app, "admin@example.com", "Admin@1234").await;
    assert!(!token.is_empty(), "token should not be empty");
}

// ── Dev endpoint tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dev_email_log_empty() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;

    let (status, body) = get(&app, "/dev/email-logs/latest").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("No email logs found"));
}

#[tokio::test]
async fn test_dev_email_log_after_login() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    post(
        &app,
        "/auth/login",
        json!({ "email": "admin@example.com", "password": "Admin@1234" }),
    )
    .await;

    let (status, body) = get(&app, "/dev/email-logs/latest").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["recipient_email"], "admin@example.com");
    assert_eq!(body["subject"], "Your 2FA Verification Code");
    assert!(body["body"]
        .as_str()
        .unwrap()
        .contains("Your verification code is:"));
}

// ── Task tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_task_no_auth() {
    let (app, _pool) = setup().await;

    let req = Request::builder()
        .method("POST")
        .uri("/tasks")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "title": "Test", "priority": "medium" }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_task_invalid_token() {
    let (app, _pool) = setup().await;

    let (status, body) = post_auth(
        &app,
        "/tasks",
        json!({ "title": "Test", "priority": "medium" }),
        "not.a.real.jwt",
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap().contains("Invalid token"));
}

#[tokio::test]
async fn test_create_task_staff_forbidden() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let staff_token = get_token(&app, "jamesbond@example.com", "Bond@1234").await;

    let (status, body) = post_auth(
        &app,
        "/tasks",
        json!({ "title": "Spy task", "priority": "high" }),
        &staff_token,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("admin"));
}

#[tokio::test]
async fn test_create_task_admin_success() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let admin_token = get_token(&app, "admin@example.com", "Admin@1234").await;

    let (status, body) = post_auth(
        &app,
        "/tasks",
        json!({
            "title": "Integration test task",
            "description": "Created during integration testing",
            "priority": "high"
        }),
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "Integration test task");
    assert_eq!(body["status"], "todo");
    assert_eq!(body["priority"], "high");
    assert!(body["id"].is_string());
    assert!(body["assigned_to"].is_null());
}

#[tokio::test]
async fn test_assign_tasks_admin_success() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let admin_token = get_token(&app, "admin@example.com", "Admin@1234").await;

    let (_, task_body) = post_auth(
        &app,
        "/tasks",
        json!({ "title": "Task to assign", "priority": "medium" }),
        &admin_token,
    )
    .await;
    let task_id = task_body["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &app,
        "/tasks/assign",
        json!({
            "task_ids": [task_id],
            "assigned_to_email": "jamesbond@example.com"
        }),
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["assigned_count"], 1);
    assert_eq!(body["assigned_to"], "jamesbond@example.com");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Successfully assigned"));
}

#[tokio::test]
async fn test_assign_tasks_user_not_found() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let admin_token = get_token(&app, "admin@example.com", "Admin@1234").await;

    let (status, body) = post_auth(
        &app,
        "/tasks/assign",
        json!({
            "task_ids": [],
            "assigned_to_email": "nobody@example.com"
        }),
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn test_assign_tasks_staff_forbidden() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let staff_token = get_token(&app, "jamesbond@example.com", "Bond@1234").await;

    let (status, body) = post_auth(
        &app,
        "/tasks/assign",
        json!({
            "task_ids": [],
            "assigned_to_email": "jamesbond@example.com"
        }),
        &staff_token,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("admin"));
}

#[tokio::test]
async fn test_view_my_tasks_no_auth() {
    let (app, _pool) = setup().await;

    let req = Request::builder()
        .method("GET")
        .uri("/tasks/view-my-tasks")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_view_my_tasks_success() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let admin_token = get_token(&app, "admin@example.com", "Admin@1234").await;

    // Create a task and assign it to staff
    let (_, task_body) = post_auth(
        &app,
        "/tasks",
        json!({ "title": "Task for Bond", "priority": "low" }),
        &admin_token,
    )
    .await;
    let task_id = task_body["id"].as_str().unwrap();

    post_auth(
        &app,
        "/tasks/assign",
        json!({
            "task_ids": [task_id],
            "assigned_to_email": "jamesbond@example.com"
        }),
        &admin_token,
    )
    .await;

    let staff_token = get_token(&app, "jamesbond@example.com", "Bond@1234").await;

    let (status, body) = get_auth(&app, "/tasks/view-my-tasks", &staff_token).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["email"], "jamesbond@example.com");
    assert_eq!(body["user"]["role"], "staff");
    assert_eq!(body["summary"]["total_assigned_tasks"], 1);
    assert_eq!(body["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(body["tasks"][0]["title"], "Task for Bond");
    assert_eq!(body["tasks"][0]["status"], "todo");
    assert_eq!(body["cache"]["hit"], false);
}

#[tokio::test]
async fn test_view_my_tasks_cache_hit() {
    let (app, pool) = setup().await;
    clean_db(&pool).await;
    post(&app, "/seed/users", json!({})).await;

    let staff_token = get_token(&app, "jamesbond@example.com", "Bond@1234").await;

    // First call — DB hit, cache miss
    let (status, body) = get_auth(&app, "/tasks/view-my-tasks", &staff_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cache"]["hit"], false);

    // Second call — served from shared Arc<DashMap> cache
    let (status, body) = get_auth(&app, "/tasks/view-my-tasks", &staff_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cache"]["hit"], true);
}
