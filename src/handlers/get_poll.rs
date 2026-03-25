use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Serialize;
use sqlx::{MySqlPool, Row};

use crate::middleware::auth::AuthUser;

#[derive(Serialize)]
pub struct PollOption {
    pub id: i32,
    pub option_text: String,
    pub vote_count: i64,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub id: String,
    pub title: String,
    pub about: Option<String>,
    pub multiple_choice: bool,
    pub closes_at: Option<String>,
    pub is_closed: bool,
    pub team_id: String,
    pub created_by: String,
    pub choices: Vec<PollOption>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub team_id: Option<String>,
}

pub async fn get_poll(
    State(pool): State<MySqlPool>,
    Path(poll_id): Path<String>,
    Extension(current_user): Extension<AuthUser>,
) -> Result<Json<PollResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Fetch Poll
    let poll_row = sqlx::query("SELECT id, title, about, multiple_choice, closes_at, status, team_id, created_by FROM polls WHERE id = ?")
        .bind(&poll_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error fetching poll".into(), team_id: None })
        ))?;

    let poll = match poll_row {
        Some(r) => r,
        None => return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "Poll not found".into(), team_id: None })
        ))
    };

    let team_id: String = poll.try_get("team_id").unwrap_or_default();

    // Check Membership
    let member_check = sqlx::query("SELECT id FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Membership verification failed".into(), team_id: None })
        ))?;

    if member_check.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse { 
                error: "You are not a member of this team".into(),
                team_id: Some(team_id)
            })
        ));
    }

    // Fetch options and counts
    let options = sqlx::query("
        SELECT c.id, c.option_text, COUNT(v.id) as vote_count 
        FROM choices c 
        LEFT JOIN votes v ON c.id = v.option_id 
        WHERE c.poll_id = ? 
        GROUP BY c.id
    ")
        .bind(&poll_id)
        .fetch_all(&pool)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to fetch choices".into(), team_id: None })
        ))?;

    let mut opts = Vec::new();
    for row in options {
        opts.push(PollOption {
            id: row.try_get("id").unwrap_or(0),
            option_text: row.try_get("option_text").unwrap_or_default(),
            vote_count: row.try_get("vote_count").unwrap_or(0),
        });
    }

    let status: String = poll.try_get("status").unwrap_or_else(|_| "open".to_string());
    let closes_at: Option<chrono::NaiveDateTime> = poll.try_get("closes_at").ok().flatten();

    let mut is_closed = status == "closed";
    if !is_closed {
        if let Some(time) = closes_at {
            if Utc::now().naive_utc() > time {
                is_closed = true;
            }
        }
    }

    let multiple_choice: bool = poll.try_get("multiple_choice").unwrap_or(false);
    let created_by: String = poll.try_get("created_by").unwrap_or_default();

    Ok(Json(PollResponse {
        id: poll.try_get("id").unwrap_or_default(),
        title: poll.try_get("title").unwrap_or_default(),
        about: poll.try_get("about").unwrap_or_default(),
        multiple_choice,
        closes_at: closes_at.map(|t| t.to_string()),
        is_closed,
        team_id,
        created_by,
        choices: opts,
    }))
}