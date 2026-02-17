//! Fiber Escrow Service
//!
//! A hold invoice based escrow system with multi-role Web UI.

mod handlers;
mod models;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use handlers::*;
use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = AppState::new();

    // Pre-register some demo users
    state.register_user("alice".to_string());
    state.register_user("bob".to_string());
    state.register_user("carol".to_string());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // User
        .route("/api/user/register", post(register_user))
        .route("/api/user/me", get(get_current_user))
        .route("/api/users", get(list_users))
        // Products
        .route("/api/products", post(create_product))
        .route("/api/products", get(list_products))
        .route("/api/products/mine", get(list_my_products))
        // Orders
        .route("/api/orders", post(create_order))
        .route("/api/orders/mine", get(list_my_orders))
        .route("/api/orders/:id/pay", post(pay_order))
        .route("/api/orders/:id/ship", post(ship_order))
        .route("/api/orders/:id/confirm", post(confirm_order))
        .route("/api/orders/:id/dispute", post(dispute_order))
        // Arbiter
        .route("/api/arbiter/disputes", get(list_disputes))
        .route("/api/arbiter/disputes/:id/resolve", post(resolve_dispute))
        // System
        .route("/api/system/tick", post(tick))
        // Health
        .route("/api/health", get(health))
        // Static files
        .fallback_service(ServeDir::new("static"))
        .layer(cors)
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Escrow service starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
