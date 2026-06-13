use axum::{extract::State, Extension, Json};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    cache::task_cache_key,
    errors::{AppError, AppResult},
    models::{
        AssignTasksRequest, CacheMeta, Claims, CreateTaskRequest, MyTasksResponse, MyTasksUser,
        Task, TaskPriority, TaskPublic, TaskStatus, TaskSummary, UserRole,
    },
    AppState,
};

pub async fn create_task(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateTaskRequest>,
) -> AppResult<Json<TaskPublic>> {
    // Only Admin can create tasks
    if claims.role != "admin" {
        return Err(AppError::Forbidden(
            "Only admin users can create tasks".to_string(),
        ));
    }

    let task_id = Uuid::new_v4();
    let now = Utc::now();
    let creator_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal("Invalid user ID in token".to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO tasks (id, title, description, status, priority, created_by_id, assigned_to_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NULL, $7, $8)
        "#,
    )
    .bind(task_id)
    .bind(&body.title)
    .bind(&body.description)
    .bind(TaskStatus::Todo)
    .bind(&body.priority)
    .bind(creator_id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    tracing::info!("Task created: {} by admin {}", task_id, claims.email);

    Ok(Json(TaskPublic {
        id: task_id,
        title: body.title,
        status: TaskStatus::Todo,
        priority: body.priority,
        assigned_to: None,
    }))
}

pub async fn assign_tasks(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AssignTasksRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Only Admin can assign tasks
    if claims.role != "admin" {
        return Err(AppError::Forbidden(
            "Only admin users can assign tasks".to_string(),
        ));
    }

    // Find target user
    let target = sqlx::query_as::<_, (Uuid, String, UserRole)>(
        "SELECT id, email, role FROM users WHERE email = $1",
    )
    .bind(&body.assigned_to_email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User {} not found", body.assigned_to_email)))?;

    let (target_user_id, target_email, _target_role) = target;
    let now = Utc::now();

    let mut assigned_count = 0usize;

    for task_id in &body.task_ids {
        let rows = sqlx::query(
            r#"
            UPDATE tasks SET assigned_to_id = $1, updated_at = $2
            WHERE id = $3 AND assigned_to_id IS NULL
            "#,
        )
        .bind(target_user_id)
        .bind(now)
        .bind(task_id)
        .execute(&state.db)
        .await?
        .rows_affected();

        if rows > 0 {
            assigned_count += 1;
        }
    }

    // Invalidate task cache for the assigned user
    let cache_key = task_cache_key(&target_user_id.to_string());
    state.cache.invalidate(&cache_key);

    tracing::info!(
        "Assigned {} tasks to {} ({})",
        assigned_count,
        target_email,
        target_user_id
    );

    Ok(Json(serde_json::json!({
        "message": format!("Successfully assigned {} task(s) to {}", assigned_count, target_email),
        "assigned_count": assigned_count,
        "assigned_to": target_email,
    })))
}

pub async fn view_my_tasks(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<MyTasksResponse>> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal("Invalid user ID in token".to_string()))?;

    let cache_key = task_cache_key(&user_id.to_string());

    // Try cache first
    if let Some(cached) = state.cache.get(&cache_key) {
        if let Ok(mut response) = serde_json::from_value::<MyTasksResponse>(cached) {
            response.cache.hit = true;
            return Ok(Json(response));
        }
    }

    // Fetch user details
    let user_row = sqlx::query_as::<_, (String, UserRole)>(
        "SELECT email, role FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let (user_email, user_role) = user_row;

    // Fetch assigned tasks with assignee email
    let rows = sqlx::query_as::<_, (Uuid, String, TaskStatus, TaskPriority, Option<String>)>(
        r#"
        SELECT t.id, t.title, t.status, t.priority, u.email as assigned_to_email
        FROM tasks t
        LEFT JOIN users u ON u.id = t.assigned_to_id
        WHERE t.assigned_to_id = $1
        ORDER BY t.created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let tasks: Vec<TaskPublic> = rows
        .into_iter()
        .map(|(id, title, status, priority, assigned_to)| TaskPublic {
            id,
            title,
            status,
            priority,
            assigned_to,
        })
        .collect();

    let total = tasks.len();

    let response = MyTasksResponse {
        user: MyTasksUser {
            email: user_email,
            role: user_role,
        },
        tasks,
        summary: TaskSummary {
            total_assigned_tasks: total,
        },
        cache: CacheMeta { hit: false },
    };

    // Store in cache (serialize with hit=false, when served from cache we flip to true)
    if let Ok(val) = serde_json::to_value(&response) {
        state.cache.set(cache_key, val, 300); // 5 min TTL
    }

    Ok(Json(response))
}