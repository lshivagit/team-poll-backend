use axum::{
    extract::{Extension, Path, State},
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::middleware::auth::AuthUser;

#[derive(Deserialize)]
pub struct CreatePollRequest {
    pub title: String,
    pub about: Option<String>,
    pub multiple_choice: Option<bool>,
    pub closes_at: Option<String>,
    pub choices: Vec<String>,
}

#[derive(Serialize)]
pub struct CreatePollResponse {
    pub poll_id: String,
    pub share_url: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub async fn create_poll(
    State(pool): State<MySqlPool>,
    Path(team_id): Path<String>,
    Extension(current_user): Extension<AuthUser>,
    Json(payload): Json<CreatePollRequest>,
) -> Result<Json<CreatePollResponse>, (StatusCode, Json<ErrorResponse>)> {

    if payload.choices.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Poll must have at least 2 choices".into(),
            }),
        ));
    }

    // Check if user is part of the team
    let member_check = sqlx::query("SELECT id FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error checking team membership".into() })
        ))?;

    if member_check.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse { error: "You are not a member of this team".into() })
        ));
    }

    let poll_id = Uuid::new_v4().to_string();

    let mut tx = pool.begin().await.map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: "Database transaction failed".into() })
    ))?;

    sqlx::query(
        "INSERT INTO polls (id, team_id, created_by, title, about, multiple_choice, closes_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&poll_id)
    .bind(&team_id)
    .bind(&current_user.user_id)
    .bind(&payload.title)
    .bind(&payload.about)
    .bind(payload.multiple_choice)
    .bind(&payload.closes_at)
    .execute(&mut *tx)
    .await
    .map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "Failed to create poll".into(),
        })
    ))?;

    for option in payload.choices {
        sqlx::query(
            "INSERT INTO choices (poll_id, option_text)
             VALUES (?, ?)"
        )
        .bind(&poll_id)
        .bind(&option)
        .execute(&mut *tx)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to insert option".into(),
            })
        ))?;
    }

    tx.commit().await.map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: "Failed to commit poll creation".into() })
    ))?;

    Ok(Json(CreatePollResponse {
        poll_id: poll_id.clone(),
        share_url: format!("http://localhost:3000/poll/{}", poll_id)
    }))
}