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
use std::sync::Arc;
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

    // Check for Fiber RPC configuration
    // Seller's node: creates invoices, checks payment status, settles/cancels
    let seller_client = if let Ok(url) = std::env::var("FIBER_SELLER_RPC_URL") {
        tracing::info!("Seller Fiber RPC enabled: {}", url);
        Some(Arc::new(fiber_core::RpcFiberClient::new(url)))
    } else {
        tracing::info!("Seller Fiber RPC not configured (set FIBER_SELLER_RPC_URL to enable)");
        None
    };

    // Buyer's node: sends payments
    let buyer_rpc_url = if let Ok(url) = std::env::var("FIBER_BUYER_RPC_URL") {
        tracing::info!("Buyer Fiber RPC enabled: {}", url);
        Some(url)
    } else {
        tracing::info!("Buyer Fiber RPC not configured (set FIBER_BUYER_RPC_URL to enable)");
        None
    };

    let state = AppState::with_fiber_clients(seller_client, buyer_rpc_url);

    // Pre-register demo users with role-based names
    state.register_user("buyer".to_string());
    let seller = state.register_user("seller".to_string());
    state.register_user("arbiter".to_string());

    // Pre-create demo products (hardcoded)
    state.create_product(
        seller.id,
        "Digital Art NFT".to_string(),
        "A unique piece of digital artwork, delivered as high-resolution PNG.".to_string(),
        1000,
    );
    state.create_product(
        seller.id,
        "E-book: Rust Programming".to_string(),
        "Comprehensive guide to Rust programming language, PDF format.".to_string(),
        500,
    );
    state.create_product(
        seller.id,
        "Music Album (MP3)".to_string(),
        "Original electronic music album, 10 tracks in MP3 format.".to_string(),
        800,
    );
    tracing::info!("Created 3 demo products for seller");

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
        .route("/api/orders/:id", get(get_order))
        .route("/api/orders/:id/invoice", post(submit_invoice))
        .route("/api/orders/:id/pay", post(pay_order))
        .route("/api/orders/:id/ship", post(ship_order))
        .route("/api/orders/:id/confirm", post(confirm_order))
        .route("/api/orders/:id/dispute", post(dispute_order))
        // Arbiter
        .route("/api/arbiter/disputes", get(list_disputes))
        .route("/api/arbiter/disputes/:id/resolve", post(resolve_dispute))
        // System
        .route("/api/system/tick", post(tick))
        // Fiber RPC Proxy (for browser to call buyer's node)
        .route("/api/fiber/send_payment", post(send_payment_proxy))
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
