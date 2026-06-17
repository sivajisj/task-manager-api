use axum::{extract::State, Json};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    models::{
        AuthTokenResponse, ErrorResponse, LoginChallengeResponse, LoginRequest, UserPublic,
        UserRole, Verify2faRequest,
    },
    services::{generate_jwt, generate_otp, hash_code, verify_code, verify_password},
    AppState,
};

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body(content = LoginRequest, description = "User credentials"),
    responses(
        (status = 200, description = "Login challenge initiated — OTP sent to email", body = LoginChallengeResponse),
        (status = 401, description = "Invalid email or password", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<LoginChallengeResponse>> {
    let user = sqlx::query_as::<_, (Uuid, String, String, UserRole)>(
        "SELECT id, email, hashed_password, role FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    let (user_id, _email, hashed_password, _role) = user;

    if !verify_password(&body.password, &hashed_password)? {
        return Err(AppError::Unauthorized("Invalid email or password".to_string()));
    }

    let otp = generate_otp();
    let otp_hash = hash_code(&otp);
    let challenge_id = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(state.config.challenge_expiry_minutes);

    sqlx::query(
        r#"
        INSERT INTO login_challenges (id, user_id, code_hash, expires_at, used, created_at)
        VALUES ($1, $2, $3, $4, false, $5)
        "#,
    )
    .bind(challenge_id)
    .bind(user_id)
    .bind(&otp_hash)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db)
    .await?;

    let email_body = format!(
        "Your verification code is: {}. It expires in {} minutes.",
        otp, state.config.challenge_expiry_minutes
    );

    sqlx::query(
        r#"
        INSERT INTO email_logs (id, recipient_email, subject, body, sent_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&body.email)
    .bind("Your 2FA Verification Code")
    .bind(&email_body)
    .bind(now)
    .execute(&state.db)
    .await?;

    tracing::info!(
        "[DEV EMAIL] To: {} | Code: {} | Challenge: {}",
        body.email,
        otp,
        challenge_id
    );

    Ok(Json(LoginChallengeResponse {
        login_challenge_id: challenge_id,
        message: "Verification code sent to your email. Check /dev/email-logs/latest for the code in development.".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/auth/verify-2fa",
    tag = "auth",
    request_body(content = Verify2faRequest, description = "Challenge ID and 6-digit OTP"),
    responses(
        (status = 200, description = "Authentication successful — JWT bearer token returned", body = AuthTokenResponse),
        (status = 400, description = "Invalid, expired, or already-used OTP", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
pub async fn verify_2fa(
    State(state): State<AppState>,
    Json(body): Json<Verify2faRequest>,
) -> AppResult<Json<AuthTokenResponse>> {
    let now = Utc::now();

    let challenge = sqlx::query_as::<_, (Uuid, Uuid, String, chrono::DateTime<Utc>, bool)>(
        "SELECT id, user_id, code_hash, expires_at, used FROM login_challenges WHERE id = $1",
    )
    .bind(body.login_challenge_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid challenge ID".to_string()))?;

    let (challenge_id, user_id, code_hash, expires_at, used) = challenge;

    if used {
        return Err(AppError::BadRequest(
            "This code has already been used".to_string(),
        ));
    }

    if now > expires_at {
        return Err(AppError::BadRequest(
            "Verification code has expired".to_string(),
        ));
    }

    if !verify_code(&body.code, &code_hash) {
        return Err(AppError::BadRequest(
            "Invalid verification code".to_string(),
        ));
    }

    sqlx::query("UPDATE login_challenges SET used = true WHERE id = $1")
        .bind(challenge_id)
        .execute(&state.db)
        .await?;

    let user = sqlx::query_as::<_, (String, String, UserRole, String)>(
        "SELECT full_name, email, role, id::text FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let (full_name, email, role, _user_id_str) = user;

    let role_str = match role {
        UserRole::Admin => "admin",
        UserRole::Staff => "staff",
    };

    let token = generate_jwt(user_id, &email, role_str, &state.config)?;

    Ok(Json(AuthTokenResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        user: UserPublic {
            id: user_id,
            full_name,
            email,
            role,
        },
    }))
}
