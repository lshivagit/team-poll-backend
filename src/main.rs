use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use std::env;
use dotenv::dotenv; 

mod handlers; 
mod routes;
mod db;
mod middleware;

use routes::polls::poll_routes;
use db::connect_db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    dotenv().ok();

    let pool = connect_db().await?;

    let app = create_router(pool);

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number");

    let addr = SocketAddr::from(([0,0,0,0], port));

    println!("Server running at http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app
    ).await?;

    Ok(())
}

fn create_router(pool: sqlx::MySqlPool) -> Router {
    Router::new()
        .merge(poll_routes())
        .with_state(pool)
        .layer(CorsLayer::permissive())
}
