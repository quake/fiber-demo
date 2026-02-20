# Fiber Escrow Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a hold invoice based escrow system where arbiter holds preimage, buyer funds are locked, and confirmation releases payment to seller.

**Architecture:** Single-service multi-role Web UI. Shared `fiber-core` library extracted from `fiber-game-core` containing Fiber trait and crypto primitives. New `fiber-escrow` workspace with escrow service.

**Tech Stack:** Rust, Axum, Tokio, Serde, MockFiberClient (from fiber-core)

---

## Phase 1: Extract fiber-core Shared Library

### Task 1.1: Create fiber-core Workspace

**Files:**
- Create: `fiber-core/Cargo.toml`
- Create: `fiber-core/src/lib.rs`
- Create: `fiber-core/src/crypto/mod.rs`
- Create: `fiber-core/src/crypto/payment.rs`
- Create: `fiber-core/src/fiber/mod.rs`
- Create: `fiber-core/src/fiber/traits.rs`
- Create: `fiber-core/src/fiber/mock.rs`

**Step 1: Create fiber-core directory structure**

```bash
mkdir -p fiber-core/src/crypto fiber-core/src/fiber
```

**Step 2: Create fiber-core/Cargo.toml**

```toml
[package]
name = "fiber-core"
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["Fiber Team"]
description = "Shared Fiber Network primitives: crypto, traits, mock client"

[dependencies]
sha2 = "0.10"
rand = "0.8"
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
thiserror = "1.0"
hex = "0.4"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
```

**Step 3: Copy crypto/payment.rs from fiber-game-core**

Copy `fiber-game/crates/fiber-game-core/src/crypto/payment.rs` to `fiber-core/src/crypto/payment.rs` (no changes needed)

**Step 4: Create fiber-core/src/crypto/mod.rs**

```rust
//! Cryptographic primitives for Fiber Network.

mod payment;

pub use payment::{PaymentHash, Preimage};
```

**Step 5: Copy fiber/traits.rs with adjusted imports**

Copy `fiber-game/crates/fiber-game-core/src/fiber/traits.rs` to `fiber-core/src/fiber/traits.rs`

Change import:
```rust
// FROM:
use crate::crypto::{PaymentHash, Preimage};
// TO:
use crate::crypto::{PaymentHash, Preimage};
```
(Same path, no change needed)

**Step 6: Copy fiber/mock.rs with adjusted imports**

Copy `fiber-game/crates/fiber-game-core/src/fiber/mock.rs` to `fiber-core/src/fiber/mock.rs`

Change import:
```rust
// FROM:
use super::traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
use crate::crypto::{PaymentHash, Preimage};
// No change needed - same paths
```

**Step 7: Create fiber-core/src/fiber/mod.rs**

```rust
//! Fiber Network client abstraction.

mod mock;
mod traits;

pub use mock::MockFiberClient;
pub use traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
```

**Step 8: Create fiber-core/src/lib.rs**

```rust
//! Fiber Core Library
//!
//! Shared primitives for Fiber Network applications:
//! - Cryptographic primitives (Preimage, PaymentHash)
//! - FiberClient trait and MockFiberClient

pub mod crypto;
pub mod fiber;

pub use crypto::{PaymentHash, Preimage};
pub use fiber::{FiberClient, FiberError, HoldInvoice, MockFiberClient, PaymentId, PaymentStatus};
```

**Step 9: Build and test fiber-core**

```bash
cd fiber-core && cargo build && cargo test
```

Expected: Build succeeds, all tests pass

**Step 10: Commit**

```bash
git add fiber-core
git commit -m "Extract fiber-core shared library from fiber-game-core"
```

---

### Task 1.2: Update fiber-game to Use fiber-core

**Files:**
- Modify: `fiber-game/Cargo.toml`
- Modify: `fiber-game/crates/fiber-game-core/Cargo.toml`
- Modify: `fiber-game/crates/fiber-game-core/src/lib.rs`
- Modify: `fiber-game/crates/fiber-game-core/src/crypto/mod.rs`
- Delete: `fiber-game/crates/fiber-game-core/src/crypto/payment.rs`
- Modify: `fiber-game/crates/fiber-game-core/src/fiber/mod.rs`
- Delete: `fiber-game/crates/fiber-game-core/src/fiber/traits.rs`
- Delete: `fiber-game/crates/fiber-game-core/src/fiber/mock.rs`

**Step 1: Update fiber-game/Cargo.toml workspace dependencies**

Add fiber-core dependency:
```toml
[workspace.dependencies]
# ... existing deps ...

# Shared core
fiber-core = { path = "../fiber-core" }
```

**Step 2: Update fiber-game-core/Cargo.toml**

Add fiber-core dependency:
```toml
[dependencies]
fiber-core = { workspace = true }
# ... keep other deps ...
```

**Step 3: Update fiber-game-core/src/crypto/mod.rs**

```rust
//! Cryptographic primitives for the game protocol.

mod commitment;
mod encrypted_preimage;
mod signature_point;

pub use commitment::{Commitment, Salt};
pub use encrypted_preimage::EncryptedPreimage;
pub use signature_point::SignaturePoint;

// Re-export from fiber-core
pub use fiber_core::{PaymentHash, Preimage};
```

**Step 4: Delete fiber-game-core/src/crypto/payment.rs**

```bash
rm fiber-game/crates/fiber-game-core/src/crypto/payment.rs
```

**Step 5: Update fiber-game-core/src/fiber/mod.rs**

```rust
//! Fiber Network client abstraction.
//!
//! Re-exports from fiber-core for backward compatibility.

pub use fiber_core::{
    FiberClient, FiberError, HoldInvoice, MockFiberClient, PaymentId, PaymentStatus,
};
```

**Step 6: Delete fiber-game-core/src/fiber/traits.rs and mock.rs**

```bash
rm fiber-game/crates/fiber-game-core/src/fiber/traits.rs
rm fiber-game/crates/fiber-game-core/src/fiber/mock.rs
```

**Step 7: Update fiber-game-core/src/lib.rs**

```rust
//! Fiber Game Core Library
//!
//! This crate provides the core protocol logic, cryptographic primitives,
//! and game definitions for the decentralized two-player game protocol.

pub mod crypto;
pub mod fiber;
pub mod games;
pub mod protocol;

pub use crypto::{Commitment, EncryptedPreimage, PaymentHash, Preimage, Salt, SignaturePoint};
pub use fiber::{FiberClient, FiberError, MockFiberClient, PaymentId, PaymentStatus};
pub use games::{GameAction, GameJudge, GameType, RpsAction};
pub use protocol::{GameId, GameResult, Player};
```

**Step 8: Build and test fiber-game**

```bash
cd fiber-game && cargo build && cargo test
```

Expected: All builds succeed, all 48 tests pass

**Step 9: Commit**

```bash
git add -A
git commit -m "Refactor fiber-game to use fiber-core shared library"
```

---

## Phase 2: Create fiber-escrow Workspace

### Task 2.1: Initialize fiber-escrow Workspace

**Files:**
- Create: `fiber-escrow/Cargo.toml`
- Create: `fiber-escrow/crates/fiber-escrow-service/Cargo.toml`
- Create: `fiber-escrow/crates/fiber-escrow-service/src/main.rs`

**Step 1: Create directory structure**

```bash
mkdir -p fiber-escrow/crates/fiber-escrow-service/src
mkdir -p fiber-escrow/crates/fiber-escrow-service/static
```

**Step 2: Create fiber-escrow/Cargo.toml**

```toml
[workspace]
members = [
    "crates/fiber-escrow-service",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["Fiber Team"]

[workspace.dependencies]
# Core
fiber-core = { path = "../fiber-core" }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP
axum = { version = "0.7", features = ["macros"] }
tower-http = { version = "0.5", features = ["fs", "cors"] }

# Async
tokio = { version = "1", features = ["full"] }

# Utils
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Step 3: Create fiber-escrow-service/Cargo.toml**

```toml
[package]
name = "fiber-escrow-service"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
description = "Fiber Escrow Service with multi-role Web UI"

[dependencies]
fiber-core = { workspace = true }
axum = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

**Step 4: Create minimal main.rs**

```rust
//! Fiber Escrow Service
//!
//! A hold invoice based escrow system with multi-role Web UI.

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build router
    let app = Router::new()
        .route("/api/health", get(health))
        .fallback_service(ServeDir::new("static"));

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Escrow service starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
```

**Step 5: Create minimal static/index.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Fiber Escrow Demo</title>
</head>
<body>
    <h1>Fiber Escrow Demo</h1>
    <p>Coming soon...</p>
</body>
</html>
```

**Step 6: Build fiber-escrow**

```bash
cd fiber-escrow && cargo build
```

Expected: Build succeeds

**Step 7: Commit**

```bash
git add fiber-escrow
git commit -m "Initialize fiber-escrow workspace with minimal service"
```

---

### Task 2.2: Implement Data Models

**Files:**
- Create: `fiber-escrow/crates/fiber-escrow-service/src/models.rs`
- Modify: `fiber-escrow/crates/fiber-escrow-service/src/main.rs`

**Step 1: Create models.rs**

```rust
//! Data models for the escrow service.

use chrono::{DateTime, Utc};
use fiber_core::{PaymentHash, Preimage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

/// Product ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProductId(pub Uuid);

impl ProductId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ProductId {
    fn default() -> Self {
        Self::new()
    }
}

/// Order ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

/// User
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub balance_shannons: i64,
}

impl User {
    pub fn new(username: String) -> Self {
        Self {
            id: UserId::new(),
            username,
            balance_shannons: 10000, // Initial balance for demo
        }
    }
}

/// Product status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductStatus {
    Available,
    Sold,
}

/// Product
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub seller_id: UserId,
    pub title: String,
    pub description: String,
    pub price_shannons: u64,
    pub status: ProductStatus,
    pub created_at: DateTime<Utc>,
}

impl Product {
    pub fn new(seller_id: UserId, title: String, description: String, price_shannons: u64) -> Self {
        Self {
            id: ProductId::new(),
            seller_id,
            title,
            description,
            price_shannons,
            status: ProductStatus::Available,
            created_at: Utc::now(),
        }
    }
}

/// Order status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    WaitingPayment,
    Funded,
    Shipped,
    Completed,
    Disputed,
    Refunded,
}

/// Dispute resolution
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeResolution {
    ToSeller,
    ToBuyer,
}

/// Dispute
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dispute {
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub resolution: Option<DisputeResolution>,
}

/// Order
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub product_id: ProductId,
    pub product_title: String,
    pub seller_id: UserId,
    pub buyer_id: UserId,
    pub amount_shannons: u64,

    // Arbiter holds preimage
    #[serde(skip_serializing)]
    pub preimage: Preimage,
    pub payment_hash: PaymentHash,

    // State
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,

    // Dispute
    pub dispute: Option<Dispute>,
}

impl Order {
    /// Create a new order with arbiter-generated preimage
    pub fn new(
        product: &Product,
        buyer_id: UserId,
        timeout_hours: i64,
    ) -> Self {
        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        Self {
            id: OrderId::new(),
            product_id: product.id,
            product_title: product.title.clone(),
            seller_id: product.seller_id,
            buyer_id,
            amount_shannons: product.price_shannons,
            preimage,
            payment_hash,
            status: OrderStatus::WaitingPayment,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(timeout_hours),
            dispute: None,
        }
    }
}
```

**Step 2: Update main.rs to include models**

Add at top of main.rs:
```rust
mod models;
```

**Step 3: Build to verify**

```bash
cd fiber-escrow && cargo build
```

Expected: Build succeeds

**Step 4: Commit**

```bash
git add -A
git commit -m "Add escrow data models (User, Product, Order)"
```

---

### Task 2.3: Implement Application State

**Files:**
- Create: `fiber-escrow/crates/fiber-escrow-service/src/state.rs`
- Modify: `fiber-escrow/crates/fiber-escrow-service/src/main.rs`

**Step 1: Create state.rs**

```rust
//! Application state management.

use crate::models::*;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
}

struct AppStateInner {
    users: HashMap<UserId, User>,
    products: HashMap<ProductId, Product>,
    orders: HashMap<OrderId, Order>,
    /// Simulated current time (for timeout testing)
    current_time: Option<DateTime<Utc>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                users: HashMap::new(),
                products: HashMap::new(),
                orders: HashMap::new(),
                current_time: None,
            })),
        }
    }

    /// Get current time (real or simulated)
    pub fn now(&self) -> DateTime<Utc> {
        self.inner
            .lock()
            .unwrap()
            .current_time
            .unwrap_or_else(Utc::now)
    }

    /// Advance simulated time by seconds
    pub fn advance_time(&self, seconds: i64) {
        let mut inner = self.inner.lock().unwrap();
        let current = inner.current_time.unwrap_or_else(Utc::now);
        inner.current_time = Some(current + chrono::Duration::seconds(seconds));
    }

    // User operations

    pub fn register_user(&self, username: String) -> User {
        let user = User::new(username);
        let mut inner = self.inner.lock().unwrap();
        inner.users.insert(user.id, user.clone());
        user
    }

    pub fn get_user(&self, id: UserId) -> Option<User> {
        self.inner.lock().unwrap().users.get(&id).cloned()
    }

    pub fn get_user_by_username(&self, username: &str) -> Option<User> {
        self.inner
            .lock()
            .unwrap()
            .users
            .values()
            .find(|u| u.username == username)
            .cloned()
    }

    pub fn list_users(&self) -> Vec<User> {
        self.inner.lock().unwrap().users.values().cloned().collect()
    }

    pub fn adjust_balance(&self, user_id: UserId, amount: i64) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(user) = inner.users.get_mut(&user_id) {
            user.balance_shannons += amount;
        }
    }

    // Product operations

    pub fn create_product(&self, seller_id: UserId, title: String, description: String, price_shannons: u64) -> Product {
        let product = Product::new(seller_id, title, description, price_shannons);
        let mut inner = self.inner.lock().unwrap();
        inner.products.insert(product.id, product.clone());
        product
    }

    pub fn get_product(&self, id: ProductId) -> Option<Product> {
        self.inner.lock().unwrap().products.get(&id).cloned()
    }

    pub fn list_available_products(&self) -> Vec<Product> {
        self.inner
            .lock()
            .unwrap()
            .products
            .values()
            .filter(|p| p.status == ProductStatus::Available)
            .cloned()
            .collect()
    }

    pub fn list_products_by_seller(&self, seller_id: UserId) -> Vec<Product> {
        self.inner
            .lock()
            .unwrap()
            .products
            .values()
            .filter(|p| p.seller_id == seller_id)
            .cloned()
            .collect()
    }

    pub fn mark_product_sold(&self, id: ProductId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(product) = inner.products.get_mut(&id) {
            product.status = ProductStatus::Sold;
        }
    }

    pub fn mark_product_available(&self, id: ProductId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(product) = inner.products.get_mut(&id) {
            product.status = ProductStatus::Available;
        }
    }

    // Order operations

    pub fn create_order(&self, product: &Product, buyer_id: UserId) -> Order {
        let order = Order::new(product, buyer_id, 24); // 24 hour timeout
        let mut inner = self.inner.lock().unwrap();
        inner.orders.insert(order.id, order.clone());
        order
    }

    pub fn get_order(&self, id: OrderId) -> Option<Order> {
        self.inner.lock().unwrap().orders.get(&id).cloned()
    }

    pub fn update_order_status(&self, id: OrderId, status: OrderStatus) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&id) {
            order.status = status;
        }
    }

    pub fn list_orders_for_user(&self, user_id: UserId) -> Vec<Order> {
        self.inner
            .lock()
            .unwrap()
            .orders
            .values()
            .filter(|o| o.buyer_id == user_id || o.seller_id == user_id)
            .cloned()
            .collect()
    }

    pub fn list_disputed_orders(&self) -> Vec<Order> {
        self.inner
            .lock()
            .unwrap()
            .orders
            .values()
            .filter(|o| o.status == OrderStatus::Disputed)
            .cloned()
            .collect()
    }

    pub fn add_dispute(&self, order_id: OrderId, reason: String) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&order_id) {
            order.dispute = Some(Dispute {
                reason,
                created_at: Utc::now(),
                resolution: None,
            });
            order.status = OrderStatus::Disputed;
        }
    }

    pub fn resolve_dispute(&self, order_id: OrderId, resolution: DisputeResolution) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&order_id) {
            if let Some(ref mut dispute) = order.dispute {
                dispute.resolution = Some(resolution);
            }
            order.status = match resolution {
                DisputeResolution::ToSeller => OrderStatus::Completed,
                DisputeResolution::ToBuyer => OrderStatus::Refunded,
            };
        }
    }

    /// Check for expired orders and auto-confirm them
    pub fn process_expired_orders(&self) -> Vec<OrderId> {
        let now = self.now();
        let mut expired = Vec::new();

        let mut inner = self.inner.lock().unwrap();
        for order in inner.orders.values_mut() {
            // Only auto-confirm shipped orders that have expired
            if order.status == OrderStatus::Shipped && order.expires_at <= now {
                order.status = OrderStatus::Completed;
                expired.push(order.id);
            }
        }

        expired
    }

    /// Get preimage for a completed/refunded order (for settlement)
    pub fn get_order_preimage(&self, order_id: OrderId) -> Option<fiber_core::Preimage> {
        let inner = self.inner.lock().unwrap();
        inner.orders.get(&order_id).map(|o| o.preimage.clone())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Update main.rs to use state**

```rust
//! Fiber Escrow Service

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod models;
mod state;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = AppState::new();

    let app = Router::new()
        .route("/api/health", get(health))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Escrow service starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
```

**Step 3: Build to verify**

```bash
cd fiber-escrow && cargo build
```

**Step 4: Commit**

```bash
git add -A
git commit -m "Add escrow application state management"
```

---

### Task 2.4: Implement API Handlers

**Files:**
- Create: `fiber-escrow/crates/fiber-escrow-service/src/handlers.rs`
- Modify: `fiber-escrow/crates/fiber-escrow-service/src/main.rs`

**Step 1: Create handlers.rs**

```rust
//! HTTP API handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::*;
use crate::state::AppState;

// ============ Request/Response types ============

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub balance_shannons: i64,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id.0,
            username: u.username,
            balance_shannons: u.balance_shannons,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub title: String,
    pub description: String,
    pub price_shannons: u64,
}

#[derive(Serialize)]
pub struct ProductResponse {
    pub id: Uuid,
    pub seller_id: Uuid,
    pub seller_username: Option<String>,
    pub title: String,
    pub description: String,
    pub price_shannons: u64,
    pub status: ProductStatus,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub product_id: Uuid,
}

#[derive(Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_title: String,
    pub seller_id: Uuid,
    pub buyer_id: Uuid,
    pub amount_shannons: u64,
    pub payment_hash: String,
    pub status: OrderStatus,
    pub created_at: String,
    pub expires_at: String,
    pub dispute: Option<DisputeResponse>,
}

#[derive(Serialize)]
pub struct DisputeResponse {
    pub reason: String,
    pub created_at: String,
    pub resolution: Option<DisputeResolution>,
}

#[derive(Deserialize)]
pub struct DisputeRequest {
    pub reason: String,
}

#[derive(Deserialize)]
pub struct ResolveDisputeRequest {
    pub resolution: String, // "seller" or "buyer"
}

#[derive(Deserialize)]
pub struct TickRequest {
    pub seconds: i64,
}

#[derive(Serialize)]
pub struct TickResponse {
    pub expired_orders: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============ Helper to get user from header ============

fn get_user_id_from_header(headers: &axum::http::HeaderMap) -> Option<UserId> {
    headers
        .get("X-User-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(UserId)
}

// ============ User handlers ============

pub async fn register_user(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Check if username already exists
    if state.get_user_by_username(&req.username).is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Username already exists"})),
        );
    }

    let user = state.register_user(req.username);
    (StatusCode::OK, Json(serde_json::json!(UserResponse::from(user))))
}

pub async fn get_current_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    match state.get_user(user_id) {
        Some(user) => (StatusCode::OK, Json(serde_json::json!(UserResponse::from(user)))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "User not found"})),
        ),
    }
}

pub async fn list_users(State(state): State<AppState>) -> impl IntoResponse {
    let users: Vec<UserResponse> = state.list_users().into_iter().map(Into::into).collect();
    Json(serde_json::json!({"users": users}))
}

// ============ Product handlers ============

pub async fn create_product(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateProductRequest>,
) -> impl IntoResponse {
    let seller_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let product = state.create_product(seller_id, req.title, req.description, req.price_shannons);
    (
        StatusCode::OK,
        Json(serde_json::json!({"product_id": product.id.0})),
    )
}

pub async fn list_products(State(state): State<AppState>) -> impl IntoResponse {
    let products: Vec<ProductResponse> = state
        .list_available_products()
        .into_iter()
        .map(|p| {
            let seller = state.get_user(p.seller_id);
            ProductResponse {
                id: p.id.0,
                seller_id: p.seller_id.0,
                seller_username: seller.map(|u| u.username),
                title: p.title,
                description: p.description,
                price_shannons: p.price_shannons,
                status: p.status,
            }
        })
        .collect();
    Json(serde_json::json!({"products": products}))
}

pub async fn list_my_products(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let seller_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let products: Vec<ProductResponse> = state
        .list_products_by_seller(seller_id)
        .into_iter()
        .map(|p| ProductResponse {
            id: p.id.0,
            seller_id: p.seller_id.0,
            seller_username: None,
            title: p.title,
            description: p.description,
            price_shannons: p.price_shannons,
            status: p.status,
        })
        .collect();
    (StatusCode::OK, Json(serde_json::json!({"products": products})))
}

// ============ Order handlers ============

fn order_to_response(order: &Order) -> OrderResponse {
    OrderResponse {
        id: order.id.0,
        product_id: order.product_id.0,
        product_title: order.product_title.clone(),
        seller_id: order.seller_id.0,
        buyer_id: order.buyer_id.0,
        amount_shannons: order.amount_shannons,
        payment_hash: order.payment_hash.to_string(),
        status: order.status,
        created_at: order.created_at.to_rfc3339(),
        expires_at: order.expires_at.to_rfc3339(),
        dispute: order.dispute.as_ref().map(|d| DisputeResponse {
            reason: d.reason.clone(),
            created_at: d.created_at.to_rfc3339(),
            resolution: d.resolution,
        }),
    }
}

pub async fn create_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateOrderRequest>,
) -> impl IntoResponse {
    let buyer_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let product_id = ProductId(req.product_id);
    let product = match state.get_product(product_id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Product not found"})),
            )
        }
    };

    if product.status != ProductStatus::Available {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Product not available"})),
        );
    }

    if product.seller_id == buyer_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Cannot buy your own product"})),
        );
    }

    // Mark product as sold
    state.mark_product_sold(product_id);

    // Create order
    let order = state.create_order(&product, buyer_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "order_id": order.id.0,
            "payment_hash": order.payment_hash.to_string(),
            "amount_shannons": order.amount_shannons,
            "expires_at": order.expires_at.to_rfc3339()
        })),
    )
}

pub async fn list_my_orders(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let orders: Vec<OrderResponse> = state
        .list_orders_for_user(user_id)
        .iter()
        .map(order_to_response)
        .collect();
    (StatusCode::OK, Json(serde_json::json!({"orders": orders})))
}

pub async fn pay_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(order_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let order_id = OrderId(order_id);
    let order = match state.get_order(order_id) {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Order not found"})),
            )
        }
    };

    if order.buyer_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Not the buyer"})),
        );
    }

    if order.status != OrderStatus::WaitingPayment {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Order not in WaitingPayment status"})),
        );
    }

    // Check buyer balance
    let buyer = state.get_user(user_id).unwrap();
    if buyer.balance_shannons < order.amount_shannons as i64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Insufficient balance"})),
        );
    }

    // Lock buyer funds (simulated hold invoice)
    state.adjust_balance(user_id, -(order.amount_shannons as i64));
    state.update_order_status(order_id, OrderStatus::Funded);

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "funded"})),
    )
}

pub async fn ship_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(order_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let order_id = OrderId(order_id);
    let order = match state.get_order(order_id) {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Order not found"})),
            )
        }
    };

    if order.seller_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Not the seller"})),
        );
    }

    if order.status != OrderStatus::Funded {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Order not in Funded status"})),
        );
    }

    state.update_order_status(order_id, OrderStatus::Shipped);

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "shipped"})),
    )
}

pub async fn confirm_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(order_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let order_id = OrderId(order_id);
    let order = match state.get_order(order_id) {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Order not found"})),
            )
        }
    };

    if order.buyer_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Not the buyer"})),
        );
    }

    if order.status != OrderStatus::Shipped {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Order not in Shipped status"})),
        );
    }

    // Release funds to seller
    state.adjust_balance(order.seller_id, order.amount_shannons as i64);
    state.update_order_status(order_id, OrderStatus::Completed);

    // Get preimage for response
    let preimage = state.get_order_preimage(order_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "completed",
            "preimage": preimage.map(|p| hex::encode(p.as_bytes()))
        })),
    )
}

pub async fn dispute_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(order_id): Path<Uuid>,
    Json(req): Json<DisputeRequest>,
) -> impl IntoResponse {
    let user_id = match get_user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing X-User-Id header"})),
            )
        }
    };

    let order_id = OrderId(order_id);
    let order = match state.get_order(order_id) {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Order not found"})),
            )
        }
    };

    if order.buyer_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Not the buyer"})),
        );
    }

    // Can only dispute funded or shipped orders
    if order.status != OrderStatus::Funded && order.status != OrderStatus::Shipped {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Cannot dispute this order"})),
        );
    }

    state.add_dispute(order_id, req.reason);

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "disputed"})),
    )
}

// ============ Arbiter handlers ============

pub async fn list_disputes(State(state): State<AppState>) -> impl IntoResponse {
    let disputes: Vec<OrderResponse> = state
        .list_disputed_orders()
        .iter()
        .map(order_to_response)
        .collect();
    Json(serde_json::json!({"disputes": disputes}))
}

pub async fn resolve_dispute(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(req): Json<ResolveDisputeRequest>,
) -> impl IntoResponse {
    let order_id = OrderId(order_id);
    let order = match state.get_order(order_id) {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Order not found"})),
            )
        }
    };

    if order.status != OrderStatus::Disputed {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Order not disputed"})),
        );
    }

    let resolution = match req.resolution.as_str() {
        "seller" => DisputeResolution::ToSeller,
        "buyer" => DisputeResolution::ToBuyer,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid resolution, use 'seller' or 'buyer'"})),
            )
        }
    };

    // Process resolution
    match resolution {
        DisputeResolution::ToSeller => {
            // Release funds to seller
            state.adjust_balance(order.seller_id, order.amount_shannons as i64);
        }
        DisputeResolution::ToBuyer => {
            // Refund buyer
            state.adjust_balance(order.buyer_id, order.amount_shannons as i64);
            // Mark product as available again
            state.mark_product_available(order.product_id);
        }
    }

    state.resolve_dispute(order_id, resolution);

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "resolved", "resolution": req.resolution})),
    )
}

// ============ System handlers ============

pub async fn tick(
    State(state): State<AppState>,
    Json(req): Json<TickRequest>,
) -> impl IntoResponse {
    state.advance_time(req.seconds);

    // Process expired orders
    let expired_order_ids = state.process_expired_orders();

    // Release funds for expired orders
    for order_id in &expired_order_ids {
        if let Some(order) = state.get_order(*order_id) {
            state.adjust_balance(order.seller_id, order.amount_shannons as i64);
        }
    }

    let expired: Vec<Uuid> = expired_order_ids.iter().map(|id| id.0).collect();
    Json(serde_json::json!(TickResponse { expired_orders: expired }))
}
```

**Step 2: Update main.rs with all routes**

```rust
//! Fiber Escrow Service

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod models;
mod state;

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

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Escrow service starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
```

**Step 3: Build to verify**

```bash
cd fiber-escrow && cargo build
```

**Step 4: Commit**

```bash
git add -A
git commit -m "Implement escrow API handlers"
```

---

### Task 2.5: Implement Web UI

**Files:**
- Modify: `fiber-escrow/crates/fiber-escrow-service/static/index.html`

**Step 1: Create complete index.html**

See full HTML in separate file due to size. Key features:
- User dropdown selector (alice/bob/carol)
- Tabs: Market, My Orders, My Products, Arbiter
- Product cards with Buy button
- Order list with status-dependent actions
- Dispute form modal
- Arbiter dispute resolution UI

**Step 2: Build and test**

```bash
cd fiber-escrow && cargo run
```

Open http://localhost:3000 and verify UI loads

**Step 3: Commit**

```bash
git add -A
git commit -m "Add escrow Web UI with multi-role support"
```

---

## Phase 3: Integration and Testing

### Task 3.1: Create E2E Test Documentation

**Files:**
- Create: `docs/escrow-e2e-test-flow.md`

Document manual test flow for:
1. Normal purchase flow (buyer confirms)
2. Timeout auto-confirm flow
3. Dispute flow (arbiter resolves)

**Step 1: Write documentation**

**Step 2: Commit**

```bash
git add docs/escrow-e2e-test-flow.md
git commit -m "Add escrow E2E test flow documentation"
```

---

### Task 3.2: Final Build Verification

**Step 1: Build all projects**

```bash
cd fiber-core && cargo build && cargo test
cd ../fiber-game && cargo build && cargo test
cd ../fiber-escrow && cargo build
```

**Step 2: Verify no warnings**

All builds should complete without warnings

**Step 3: Final commit**

```bash
git add -A
git commit -m "Complete fiber-escrow implementation"
```

---

## Summary

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| Phase 1 | Extract fiber-core | 30 min |
| Phase 2 | Build fiber-escrow service | 90 min |
| Phase 3 | Integration & testing | 30 min |

**Total: ~2.5 hours**
