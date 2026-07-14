use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::database::pool::DbPool;

#[derive(Clone)]
pub struct HealthState {
    pool: DbPool,
}

impl HealthState {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub fn routes(pool: DbPool) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(HealthState::new(pool))
}

async fn live() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn ready(State(state): State<HealthState>) -> (StatusCode, Json<HealthResponse>) {
    if state.pool.ping().await.is_ok() {
        (StatusCode::OK, Json(HealthResponse { status: "ready" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse { status: "unavailable" }),
        )
    }
}
