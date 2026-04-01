use std::env;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};

pub async fn connect_db() -> Result<MySqlPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set");

    MySqlPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .connect(&database_url)
        .await
}
