use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::Stream;
use serde::Serialize;
use sqlx::{MySqlPool, Row};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::middleware::auth::AuthUser;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn stream_results(
    Path(poll_id): Path<String>,
    State(pool): State<MySqlPool>,
    Extension(current_user): Extension<AuthUser>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    
    // Check membership before opening the stream
    let poll_row = sqlx::query("SELECT team_id FROM polls WHERE id = ?")
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

    let interval = tokio::time::interval(Duration::from_secs(2));

    let stream = IntervalStream::new(interval).then(move |_| {
        let pool = pool.clone();
        let poll_id = poll_id.clone();

        async move {
            let rows = sqlx::query(
                r#"
                SELECT 
                    c.id as choice_id,
                    c.option_text,
                    COUNT(v.id) as votes
                FROM choices c
                LEFT JOIN votes v ON c.id = v.option_id
                WHERE c.poll_id = ?
                GROUP BY c.id
                "#
            )
            .bind(&poll_id)
            .fetch_all(&pool)
            .await;

            match rows {
                Ok(data) => {
                    let mut total: i64 = 0;
                    let mut temp_results = Vec::new();

                    for row in data {
                        let votes: i64 = row.try_get("votes").unwrap_or(0);
                        let choice_id: i32 = row.try_get("choice_id").unwrap_or(0);
                        let option_text: String = row.try_get("option_text").unwrap_or_default();
                        
                        total += votes;
                        temp_results.push((choice_id, option_text, votes));
                    }

                    let mut results = Vec::new();
                    for (choice_id, text, votes) in temp_results {
                        let percent = if total == 0 {
                            0.0
                        } else {
                            (votes as f64 / total as f64) * 100.0
                        };

                        results.push(serde_json::json!({
                            "choice_id": choice_id,
                            "text": text,
                            "votes": votes,
                            "percentage": percent
                        }));
                    }

                    let payload = serde_json::json!({
                        "total_votes": total,
                        "results": results
                    });

                    Ok(Event::default().json_data(payload).unwrap())
                }
                Err(_) => Ok(Event::default().data("error")),
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}