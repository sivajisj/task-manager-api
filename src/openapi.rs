use utoipa::{
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
    Modify, OpenApi,
};

use crate::{handlers, models};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Task Manager API",
        version = "1.0.0",
        description = "Task management REST API with JWT authentication and email-based 2FA.\n\n\
            **Quickstart:**\n\
            1. `POST /seed/users` — create the default admin and staff accounts\n\
            2. `POST /auth/login` — submit credentials, receive a `login_challenge_id`\n\
            3. `GET /dev/email-logs/latest` — read the OTP from the email body\n\
            4. `POST /auth/verify-2fa` — exchange challenge + OTP for a JWT\n\
            5. Use the JWT as `Authorization: Bearer <token>` on protected routes"
    ),
    paths(
        handlers::auth::login,
        handlers::auth::verify_2fa,
        handlers::seed::seed_users,
        handlers::dev::latest_email_log,
        handlers::tasks::create_task,
        handlers::tasks::assign_tasks,
        handlers::tasks::view_my_tasks,
    ),
    components(
        schemas(
            models::UserRole,
            models::TaskStatus,
            models::TaskPriority,
            models::UserPublic,
            models::LoginRequest,
            models::LoginChallengeResponse,
            models::Verify2faRequest,
            models::AuthTokenResponse,
            models::CreateTaskRequest,
            models::AssignTasksRequest,
            models::AssignTasksResponse,
            models::TaskPublic,
            models::MyTasksResponse,
            models::MyTasksUser,
            models::TaskSummary,
            models::CacheMeta,
            models::SeedResponse,
            models::ErrorResponse,
            handlers::dev::EmailLogResponse,
        )
    ),
    tags(
        (name = "auth", description = "Authentication — login with credentials, complete 2FA, receive JWT"),
        (name = "tasks", description = "Task management — create, assign, and view tasks (JWT required)"),
        (name = "seed", description = "Database seeding — create default users for testing"),
        (name = "dev", description = "Development utilities — inspect email logs to retrieve OTP codes"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
