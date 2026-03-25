use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::middleware::auth::AuthUser;

#[derive(Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamResponse {
    pub id: String,
    pub name: String,
    pub created_by: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn create_team(
    State(pool): State<MySqlPool>,
    Extension(current_user): Extension<AuthUser>,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<Json<TeamResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    let team_id = Uuid::new_v4().to_string();

    let mut tx = pool.begin().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error".into() }),
        )
    })?;

    // Create team
    sqlx::query(
        "INSERT INTO teams (id, name, created_by) VALUES (?, ?, ?)"
    )
    .bind(&team_id)
    .bind(&payload.name)
    .bind(&current_user.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to create team".into() }),
        )
    })?;

    // Add user as admin
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'admin')"
    )
    .bind(&team_id)
    .bind(&current_user.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to add as admin".into() }),
        )
    })?;

    tx.commit().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to commit transaction".into() }),
        )
    })?;

    Ok(Json(TeamResponse {
        id: team_id,
        name: payload.name,
        created_by: Some(current_user.user_id),
    }))
}

pub async fn list_teams(
    State(pool): State<MySqlPool>,
    Extension(current_user): Extension<AuthUser>,
) -> Result<Json<Vec<TeamResponse>>, (StatusCode, Json<ErrorResponse>)> {
    
    let teams = sqlx::query_as::<_, TeamResponse>(
        r#"
        SELECT t.id, t.name, t.created_by
        FROM teams t
        JOIN team_members tm ON t.id = tm.team_id
        WHERE tm.user_id = ?
        "#
    )
    .bind(&current_user.user_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to fetch teams".into() }),
        )
    })?;

    Ok(Json(teams))
}
pub async fn join_team(
    State(pool): State<MySqlPool>,
    Extension(current_user): Extension<AuthUser>,
    Path(team_id): Path<String>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Check if already a member
    let existing = sqlx::query("SELECT id FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error checking membership".into() })
        ))?;

    if existing.is_some() {
        return Ok(Json(SuccessResponse { message: "Already a member".into() }));
    }

    // Join team as member
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'member')"
    )
    .bind(&team_id)
    .bind(&current_user.user_id)
    .execute(&pool)
    .await
    .map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: "Failed to join team. Ensure the team ID is correct.".into() })
    ))?;

    Ok(Json(SuccessResponse { message: "Successfully joined team".into() }))
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub message: String,
}
