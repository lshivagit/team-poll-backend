use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, Row};

use crate::middleware::auth::AuthUser;

#[derive(Deserialize)]
pub struct ChoiceEdit {
    pub id: Option<i32>,
    pub option_text: String,
}

#[derive(Deserialize)]
pub struct EditPollRequest {
    pub title: String,
    pub about: Option<String>,
    pub multiple_choice: bool,
    pub closes_at: Option<String>,
    pub choices: Vec<ChoiceEdit>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

pub async fn edit_poll(
    State(pool): State<MySqlPool>,
    Path(poll_id): Path<String>,
    Extension(current_user): Extension<AuthUser>,
    Json(payload): Json<EditPollRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Check if the poll exists and its status
    let poll_row = sqlx::query("SELECT created_by, status, closes_at FROM polls WHERE id = ?")
        .bind(&poll_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Database error fetching poll".into() }),
            )
        })?;

    let poll = match poll_row {
        Some(r) => r,
        None => return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "Poll not found".into() }),
        ))
    };

    let creator_id: String = poll.try_get("created_by").unwrap_or_default();
    let status: String = poll.try_get("status").unwrap_or_else(|_| "open".to_string());
    let closes_at: Option<chrono::NaiveDateTime> = poll.try_get("closes_at").ok().flatten();

    // Check if closed by status or time
    if status == "closed" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Cannot edit a closed poll".into() }),
        ));
    }

    if let Some(close_time) = closes_at {
        if chrono::Utc::now().naive_utc() > close_time {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "Cannot edit a poll after its closing time has passed".into() }),
            ));
        }
    }

    println!("Edit Debug: Creator ID in DB: '{}', User ID from JWT: '{}'", creator_id, current_user.user_id);

    // Only the creator can edit the poll
    if creator_id != current_user.user_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse { error: format!("Only the creator can edit this poll (Creator: {}, You: {})", creator_id, current_user.user_id) }),
        ));
    }

    // Check if there are any votes
    let vote_count_row = sqlx::query("SELECT COUNT(*) as count FROM votes WHERE poll_id = ?")
        .bind(&poll_id)
        .fetch_one(&pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Failed to load vote count".into() }),
            )
        })?;

    let vote_count: i64 = vote_count_row.try_get("count").unwrap_or(0);

    if vote_count > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Poll cannot be edited after voting begins".into() }),
        ));
    }

    let mut tx = pool.begin().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Transaction failed".into() }),
        )
    })?;

    // Update poll details
    sqlx::query(
        "UPDATE polls SET title = ?, about = ?, multiple_choice = ?, closes_at = ? WHERE id = ?"
    )
    .bind(&payload.title)
    .bind(&payload.about)
    .bind(payload.multiple_choice)
    .bind(&payload.closes_at)
    .bind(&poll_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to update poll".into() }),
        )
    })?;

    // Handle existing choices
    let existing_choices = sqlx::query("SELECT id FROM choices WHERE poll_id = ?")
        .bind(&poll_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Failed to fetch existing choices".into() }),
            )
        })?;

    let mut existing_ids: Vec<i32> = existing_choices.into_iter().map(|c| c.try_get("id").unwrap_or(0)).collect();

    for choice in &payload.choices {
        match choice.id {
            Some(choice_id) => {
                // Update existing choice
                sqlx::query("UPDATE choices SET option_text = ? WHERE id = ? AND poll_id = ?")
                    .bind(&choice.option_text)
                    .bind(choice_id)
                    .bind(&poll_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse { error: "Failed to update choice".into() }),
                        )
                    })?;
                
                existing_ids.retain(|id| *id != choice_id);
            }
            None => {
                // Insert new choice
                sqlx::query("INSERT INTO choices (poll_id, option_text) VALUES (?, ?)")
                    .bind(&poll_id)
                    .bind(&choice.option_text)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse { error: "Failed to insert new choice".into() }),
                        )
                    })?;
            }
        }
    }

    // Delete removed choices
    for remove_id in existing_ids {
        sqlx::query("DELETE FROM choices WHERE id = ? AND poll_id = ?")
            .bind(remove_id)
            .bind(&poll_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: "Failed to delete old choices".into() }),
                )
            })?;
    }

    tx.commit().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to commit changes".into() }),
        )
    })?;

    Ok(Json(SuccessResponse { message: "Poll updated successfully".into() }))
}