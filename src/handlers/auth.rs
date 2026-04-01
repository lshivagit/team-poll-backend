use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::env;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct AuthPayload {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn register(
    State(pool): State<MySqlPool>,
    Json(payload): Json<AuthPayload>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Hash password

    let password_hash = hash(&payload.password, DEFAULT_COST).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Failed to hash password".into() }),
        )
    })?;

    let user_id = Uuid::new_v4().to_string();

    // Insert user

    let res = sqlx::query(
        "INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)"
    )
    .bind(&user_id)
    .bind(&payload.email)
    .bind(&password_hash)
    .execute(&pool)
    .await;

    if let Err(_) = res {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "User already exists or email is invalid".into() }),
        ));
    }

    let token = generate_jwt(&user_id)?;

    Ok(Json(AuthResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        user_id,
    }))
}

pub async fn login(
    State(pool): State<MySqlPool>,
    Json(payload): Json<AuthPayload>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Find user
    let user_row = sqlx::query(
        "SELECT id, password_hash FROM users WHERE email = ?"
    )
    .bind(&payload.email)
    .fetch_optional(&pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Database error".into() }),
        )
    })?;

    let user = match user_row {
        Some(row) => {
            use sqlx::Row;
            let id: String = row.get("id");
            let password_hash: String = row.get("password_hash");
            (id, password_hash)
        },
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { error: "Invalid credentials".into() }),
        )),
    };

    // Verify password

    let is_valid = verify(&payload.password, &user.1).unwrap_or(false);
    
    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { error: "Invalid credentials".into() }),
        ));
    }

    let user_id = user.0;
    let token = generate_jwt(&user_id)?;

    Ok(Json(AuthResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        user_id,
    }))
}

fn generate_jwt(user_id: &str) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    
    let now = Utc::now();
    let expire: chrono::Duration = Duration::hours(24);
    let Claims = Claims {
        sub: user_id.to_string(),
        exp: (now + expire).timestamp() as usize,
    };

    encode(
        &Header::default(),
        &Claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Error creating token".into() }),
        )
    })
}
