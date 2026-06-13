use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{errors::AppResult, AppState};

#[derive(Debug, Serialize)]
pub struct EmailLogResponse {
    pub id: Uuid,
    pub recipient_email: String,
    pub subject: String,
    pub body: String,
    pub sent_at: DateTime<Utc>,
}

pub async fn latest_email_log(
    State(state): State<AppState>,
) -> AppResult<Json<EmailLogResponse>> {
    let row = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(
        r#"
        SELECT id, recipient_email, subject, body, sent_at
        FROM email_logs
        ORDER BY sent_at DESC
        LIMIT 1
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| crate::errors::AppError::NotFound("No email logs found".to_string()))?;

    let (id, recipient_email, subject, body, sent_at) = row;

    Ok(Json(EmailLogResponse {
        id,
        recipient_email,
        subject,
        body,
        sent_at,
    }))
}