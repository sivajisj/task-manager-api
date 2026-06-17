use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ---- Enums ----

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Staff,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Todo,
    #[serde(rename = "in_progress")]
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "task_priority", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}

// ---- User ----

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub hashed_password: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---- Task ----

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_by_id: Uuid,
    pub assigned_to_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---- LoginChallenge ----

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoginChallenge {
    pub id: Uuid,
    pub user_id: Uuid,
    pub code_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

// ---- EmailLog ----

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailLog {
    pub id: Uuid,
    pub recipient_email: String,
    pub subject: String,
    pub body: String,
    pub sent_at: DateTime<Utc>,
}

// ---- Request/Response DTOs ----

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Registered user email address
    pub email: String,
    /// User password
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginChallengeResponse {
    /// UUID to use in the /auth/verify-2fa step
    pub login_challenge_id: Uuid,
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Verify2faRequest {
    /// Challenge ID returned from /auth/login
    pub login_challenge_id: Uuid,
    /// 6-digit OTP from the email log
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthTokenResponse {
    /// JWT bearer token — include as `Authorization: Bearer <token>`
    pub access_token: String,
    pub token_type: String,
    pub user: UserPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPublic {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub role: UserRole,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        UserPublic {
            id: u.id,
            full_name: u.full_name,
            email: u.email,
            role: u.role,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignTasksRequest {
    /// List of task UUIDs to assign
    pub task_ids: Vec<Uuid>,
    /// Email of the staff user to assign the tasks to
    pub assigned_to_email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssignTasksResponse {
    pub message: String,
    pub assigned_count: usize,
    pub assigned_to: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TaskPublic {
    pub id: Uuid,
    pub title: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MyTasksResponse {
    pub user: MyTasksUser,
    pub tasks: Vec<TaskPublic>,
    pub summary: TaskSummary,
    pub cache: CacheMeta,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MyTasksUser {
    pub email: String,
    pub role: UserRole,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TaskSummary {
    pub total_assigned_tasks: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CacheMeta {
    /// true if the response was served from the in-memory cache
    pub hit: bool,
}

// ---- JWT Claims ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

// ---- Seed ----

#[derive(Debug, Serialize, ToSchema)]
pub struct SeedResponse {
    pub message: String,
    pub admin: UserPublic,
    pub staff: UserPublic,
}

// ---- Shared error response shape ----

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
