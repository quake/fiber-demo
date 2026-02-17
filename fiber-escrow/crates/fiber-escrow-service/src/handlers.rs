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
    pub balance_sat: i64,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id.0,
            username: u.username,
            balance_sat: u.balance_sat,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub title: String,
    pub description: String,
    pub price_sat: u64,
}

#[derive(Serialize)]
pub struct ProductResponse {
    pub id: Uuid,
    pub seller_id: Uuid,
    pub seller_username: Option<String>,
    pub title: String,
    pub description: String,
    pub price_sat: u64,
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
    pub amount_sat: u64,
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
    (
        StatusCode::OK,
        Json(serde_json::json!(UserResponse::from(user))),
    )
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
        Some(user) => (
            StatusCode::OK,
            Json(serde_json::json!(UserResponse::from(user))),
        ),
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

    let product = state.create_product(seller_id, req.title, req.description, req.price_sat);
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
                price_sat: p.price_sat,
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
            price_sat: p.price_sat,
            status: p.status,
        })
        .collect();
    (
        StatusCode::OK,
        Json(serde_json::json!({"products": products})),
    )
}

// ============ Order handlers ============

fn order_to_response(order: &Order) -> OrderResponse {
    OrderResponse {
        id: order.id.0,
        product_id: order.product_id.0,
        product_title: order.product_title.clone(),
        seller_id: order.seller_id.0,
        buyer_id: order.buyer_id.0,
        amount_sat: order.amount_sat,
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
            "amount_sat": order.amount_sat,
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
    if buyer.balance_sat < order.amount_sat as i64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Insufficient balance"})),
        );
    }

    // Lock buyer funds (simulated hold invoice)
    state.adjust_balance(user_id, -(order.amount_sat as i64));
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
    state.adjust_balance(order.seller_id, order.amount_sat as i64);
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
            state.adjust_balance(order.seller_id, order.amount_sat as i64);
        }
        DisputeResolution::ToBuyer => {
            // Refund buyer
            state.adjust_balance(order.buyer_id, order.amount_sat as i64);
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

pub async fn tick(State(state): State<AppState>, Json(req): Json<TickRequest>) -> impl IntoResponse {
    state.advance_time(req.seconds);

    // Process expired orders
    let expired_order_ids = state.process_expired_orders();

    // Release funds for expired orders
    for order_id in &expired_order_ids {
        if let Some(order) = state.get_order(*order_id) {
            state.adjust_balance(order.seller_id, order.amount_sat as i64);
        }
    }

    let expired: Vec<Uuid> = expired_order_ids.iter().map(|id| id.0).collect();
    Json(serde_json::json!(TickResponse { expired_orders: expired }))
}
