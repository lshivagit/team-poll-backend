use axum::{
    http::StatusCode,
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct PingResponse {
    pub message: String,
}

pub async fn ping() -> (StatusCode, Json<PingResponse>) {
    (StatusCode::OK, Json(PingResponse { message: "pong".to_string() }))
}
