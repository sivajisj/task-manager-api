use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{auth, dev, seed, tasks},
    middleware::auth_middleware,
    AppState,
};

pub fn create_router(state: AppState) -> Router {
    // Protected routes (require JWT)
    let protected = Router::new()
        .route("/tasks", post(tasks::create_task))
        .route("/tasks/assign", post(tasks::assign_tasks))
        .route("/tasks/view-my-tasks", get(tasks::view_my_tasks))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes
    let public = Router::new()
        .route("/seed/users", post(seed::seed_users))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-2fa", post(auth::verify_2fa))
        .route("/dev/email-logs/latest", get(dev::latest_email_log));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(state)
}