use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use sqlx::{MySqlPool, Row};

use crate::middleware::auth::AuthUser;

#[derive(Serialize)]
pub struct PollListItem {
    pub id: String,
    pub title: String,
    pub about: Option<String>,
    pub closes_at: Option<String>,
    pub status: String,
    pub created_by: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn list_polls(
    State(pool): State<MySqlPool>,
    Path(team_id): Path<String>,
    Extension(current_user): Extension<AuthUser>,
) -> Result<Json<Vec<PollListItem>>, (StatusCode, Json<ErrorResponse>)> {
    
    // Check if the user is part of the requested team
    let member_check = sqlx::query("SELECT id FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&current_user.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Database error checking membership".into() }),
            )
        })?;

    if member_check.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse { error: "You are not a member of this team".into() }),
        ));
    }

    // Fetch all polls for this team
    let rows = sqlx::query("SELECT id, title, about, closes_at, status, created_by FROM polls WHERE team_id = ? ORDER BY created_at DESC")
        .bind(&team_id)
        .fetch_all(&pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Failed to load polls".into() }),
            )
        })?;

    let mut polls = Vec::new();
    
    for row in rows {
        let closes_at: Option<chrono::NaiveDateTime> = row.try_get("closes_at").ok().flatten();
        
        polls.push(PollListItem {
            id: row.try_get("id").unwrap_or_default(),
            title: row.try_get("title").unwrap_or_default(),
            about: row.try_get("about").unwrap_or_default(),
            closes_at: closes_at.map(|t| t.to_string()),
            status: row.try_get("status").unwrap_or_else(|_| "open".to_string()),
            created_by: row.try_get("created_by").unwrap_or_default(),
        });
    }

    Ok(Json(polls))
}