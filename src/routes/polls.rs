use axum::{
    routing::{get, post, put},
    Router, middleware
};
use sqlx::MySqlPool;

use crate::handlers::{
    auth::{login, register},
    create_poll::create_poll,
    get_poll::get_poll,
    vote::vote,
    teams::{create_team, list_teams, join_team},
    stream_results::stream_results,
    list_polls::list_polls,
    edit_poll::edit_poll,
    health::ping,

};
use crate::middleware::auth::auth_middleware;

pub fn poll_routes() -> Router<MySqlPool> {
    
    let public_routes = Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/ping", get(ping));


    let protected_routes = Router::new()
        .route("/teams", post(create_team).get(list_teams))
        .route("/teams/:team_id/polls", post(create_poll).get(list_polls))
        .route("/teams/:team_id/join", post(join_team))
        .route("/polls/:id", get(get_poll))
        .route("/polls/:id/edit", put(edit_poll))
        .route("/polls/:id/vote", post(vote))
        .route("/polls/:id/stream", get(stream_results))
        .route_layer(middleware::from_fn(auth_middleware));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
}