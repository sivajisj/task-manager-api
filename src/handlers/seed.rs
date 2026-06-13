use axum::{extract::State, Json};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    models::{SeedResponse, UserPublic, UserRole},
    services::{hash_password},
    AppState,
};

pub async fn seed_users(State(state): State<AppState>) -> AppResult<Json<SeedResponse>> {
    let now = Utc::now();

    // Check if already seeded
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1 LIMIT 1")
            .bind("admin@example.com")
            .fetch_optional(&state.db)
            .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "Users already seeded. Drop the users table or reset the DB to re-seed.".to_string(),
        ));
    }

    let admin_id = Uuid::new_v4();
    let staff_id = Uuid::new_v4();

    let admin_hash = hash_password("Admin@1234")?;
    let staff_hash = hash_password("Bond@1234")?;

    sqlx::query(
        r#"
        INSERT INTO users (id, full_name, email, hashed_password, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(admin_id)
    .bind("Admin User")
    .bind("admin@example.com")
    .bind(&admin_hash)
    .bind(UserRole::Admin)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO users (id, full_name, email, hashed_password, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(staff_id)
    .bind("James Bond")
    .bind("jamesbond@example.com")
    .bind(&staff_hash)
    .bind(UserRole::Staff)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    tracing::info!("Seeded Admin (admin@example.com / Admin@1234) and James Bond (jamesbond@example.com / Bond@1234)");

    Ok(Json(SeedResponse {
        message: "Users seeded successfully".to_string(),
        admin: UserPublic {
            id: admin_id,
            full_name: "Admin User".to_string(),
            email: "admin@example.com".to_string(),
            role: UserRole::Admin,
        },
        staff: UserPublic {
            id: staff_id,
            full_name: "James Bond".to_string(),
            email: "jamesbond@example.com".to_string(),
            role: UserRole::Staff,
        },
    }))
}