use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    handlers::{auth, dev, seed, tasks},
    middleware::auth_middleware,
    openapi::ApiDoc,
    AppState,
};

pub fn create_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/tasks", post(tasks::create_task))
        .route("/tasks/assign", post(tasks::assign_tasks))
        .route("/tasks/view-my-tasks", get(tasks::view_my_tasks))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let public = Router::new()
        .route("/seed/users", post(seed::seed_users))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-2fa", post(auth::verify_2fa))
        .route("/dev/email-logs/latest", get(dev::latest_email_log));

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(public)
        .merge(protected)
        .with_state(state)
}
