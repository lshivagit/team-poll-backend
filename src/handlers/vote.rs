use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, Row};

use crate::middleware::auth::AuthUser;

#[derive(Deserialize)]
pub struct VoteRequest {
    pub option_ids: Vec<i32>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

pub async fn vote(
    State(pool): State<MySqlPool>,
    Path(poll_id): Path<String>,
    Extension(current_user): Extension<AuthUser>,
    Json(payload): Json<VoteRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Fetch poll constraints
    let poll_row = sqlx::query("SELECT team_id, multiple_choice, status, closes_at FROM polls WHERE id = ?")
        .bind(&poll_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error checking poll".into() }),
        ))?;

    let poll = match poll_row {
        Some(r) => r,
        None => return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "Poll not found".into() }),
        ))
    };

    let team_id: String = poll.try_get("team_id").unwrap_or_default();
    let multiple_choice: bool = poll.try_get("multiple_choice").unwrap_or(false);
    let status: String = poll.try_get("status").unwrap_or_else(|_| "open".to_string());
    let closes_at: Option<chrono::NaiveDateTime> = poll.try_get("closes_at").ok().flatten();

    // Check team membership
    let member_check = sqlx::query("SELECT id FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error checking membership".into() }),
        ))?;

    if member_check.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse { error: "You are not a member of this team".into() }),
        ));
    }

    // Check if open
    if status == "closed" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Poll is closed".into() }),
        ));
    }

    if let Some(close_time) = closes_at {
        let now = Utc::now().naive_utc();
        if now > close_time {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "Poll is closed".into() }),
            ));
        }
    }

    // Checking if user already voted (if not multiple choice, checking globally for poll)
    let existing_vote = sqlx::query("SELECT id FROM votes WHERE poll_id = ? AND user_id = ?")
        .bind(&poll_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Vote check error".into() }),
        ))?;

    if !multiple_choice && existing_vote.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse { error: "Already voted in this single-choice poll".into() }),
        ));
    }

    if payload.option_ids.is_empty() {
         return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "No option selected".into() }),
        ));
    }

    if !multiple_choice && payload.option_ids.len() > 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Cannot vote for multiple options".into() }),
        ));
    }

    // Insert votes (DB unique indexes will prevent duplicates on (poll_id, user_id, option_id))
    let mut tx = pool.begin().await.map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: "Transaction failed".into() }),
    ))?;

    for option in payload.option_ids {
        let res = sqlx::query("INSERT INTO votes (poll_id, option_id, user_id) VALUES (?, ?, ?)")
            .bind(&poll_id)
            .bind(option)
            .bind(&current_user.user_id)
            .execute(&mut *tx)
            .await;

        if res.is_err() {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse { error: "Already voted for an option".into() }),
            ));
        }
    }

    tx.commit().await.map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: "Save vote failed".into() }),
    ))?;

    Ok(Json(SuccessResponse { message: "Vote recorded".into() }))
}