//! HTTP API handlers.
//!
//! All Fiber node interactions are handled by the frontend.
//! The backend manages order state and reveals preimage when appropriate.

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
    /// Preimage (hex string with 0x prefix) - buyer generates this secretly
    /// Escrow stores it and computes payment_hash for the invoice
    pub preimage: String,
}

#[derive(Deserialize)]
pub struct SubmitInvoiceRequest {
    /// Hold invoice string created by seller
    pub invoice: String,
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
    pub invoice_string: Option<String>,
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
pub struct ConfirmOrderRequest {
    // Preimage is no longer needed - escrow already holds it from order creation
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

    let product = state.create_product(seller_id, req.title, req.description, req.price_shannons);
    (
        StatusCode::OK,
        Json(serde_json::json!({"product_id": product.id.0})),
    )
}

pub async fn list_products(State(state): State<AppState>) -> impl IntoResponse {
    let mut products = Vec::new();
    for p in state.list_available_products() {
        let seller = state.get_user(p.seller_id);
        products.push(ProductResponse {
            id: p.id.0,
            seller_id: p.seller_id.0,
            seller_username: seller.map(|u| u.username),
            title: p.title,
            description: p.description,
            price_shannons: p.price_shannons,
            status: p.status,
        });
    }
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
        amount_shannons: order.amount_shannons,
        payment_hash: order.payment_hash.to_hex(),
        invoice_string: order.invoice_string.clone(),
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

    // Parse preimage from hex and compute payment_hash
    let preimage = match fiber_core::Preimage::from_hex(&req.preimage) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid preimage format, expected hex string"})),
            )
        }
    };
    let payment_hash = preimage.payment_hash();

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

    if product.seller_id == buyer_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Cannot buy your own product"})),
        );
    }

    // Create order with computed payment_hash
    let order = state.create_order(&product, buyer_id, payment_hash);

    // Store preimage immediately (escrow holds it for timeout/dispute settlement)
    tracing::info!(
        "Storing preimage for order {}: preimage_hash={}, order_payment_hash={}",
        order.id.0,
        preimage.payment_hash().to_hex(),
        order.payment_hash.to_hex()
    );
    state.set_revealed_preimage(order.id, preimage);

    // No Fiber RPC calls — seller's frontend will create the hold invoice
    // using the payment_hash, and submit it back via /api/orders/:id/invoice

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "order_id": order.id.0,
            "payment_hash": order.payment_hash.to_hex(),
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

pub async fn get_order(
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

    // Only buyer or seller can view order details
    if order.buyer_id != user_id && order.seller_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Not authorized to view this order"})),
        );
    }

    // Include preimage for seller if order is completed (for Fiber settlement)
    let mut response = serde_json::json!(order_to_response(&order));
    
    if order.seller_id == user_id && order.status == OrderStatus::Completed {
        if let Some(preimage) = state.get_revealed_preimage(order_id) {
            response["preimage"] = serde_json::json!(format!("0x{}", hex::encode(preimage.as_bytes())));
        }
    }

    (StatusCode::OK, Json(response))
}

pub async fn submit_invoice(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(order_id): Path<Uuid>,
    Json(req): Json<SubmitInvoiceRequest>,
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

    // Only seller can submit invoice
    if order.seller_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Only seller can submit invoice"})),
        );
    }

    // Can only submit invoice for orders waiting payment
    if order.status != OrderStatus::WaitingPayment {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Order not in WaitingPayment status"})),
        );
    }

    // Validate invoice is not empty
    if req.invoice.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invoice cannot be empty"})),
        );
    }

    state.set_order_invoice(order_id, req.invoice);

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "invoice_submitted"})),
    )
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

    // Require invoice to be submitted before payment can be confirmed
    if order.invoice_string.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Seller has not submitted invoice yet"})),
        );
    }

    // No Fiber RPC calls — buyer's frontend sends payment directly to their node.
    // This endpoint is called after the buyer's frontend confirms payment was sent.

    // Update order status to funded
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
    Json(_req): Json<ConfirmOrderRequest>,
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

    // Get preimage from escrow storage (stored at order creation)
    let preimage = match state.get_revealed_preimage(order_id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Preimage not found in escrow"})),
            )
        }
    };

    // Debug: verify preimage matches payment_hash
    tracing::info!(
        "Settling order {}: preimage_hash={}, order_payment_hash={}",
        order_id.0,
        preimage.payment_hash().to_hex(),
        order.payment_hash.to_hex()
    );

    // Mark order as completed
    state.update_order_status(order_id, OrderStatus::Completed);

    // No Fiber RPC calls — seller's frontend will call settle_invoice
    // after seeing the preimage in the order details.
    tracing::info!("Order {} completed, preimage available for seller settlement", order_id.0);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "completed"
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

    // Return preimage if resolving to seller (seller's frontend will call settle_invoice)
    // If resolving to buyer, seller's frontend should call cancel_invoice
    let mut preimage_hex: Option<String> = None;

    match resolution {
        DisputeResolution::ToSeller => {
            if let Some(preimage) = state.get_revealed_preimage(order_id) {
                preimage_hex = Some(format!("0x{}", hex::encode(preimage.as_bytes())));
                tracing::info!(
                    "Dispute resolved to seller for order {} - preimage available for settlement",
                    order_id.0
                );
            } else {
                tracing::warn!(
                    "No preimage found for disputed order {} - cannot provide for settlement",
                    order_id.0
                );
            }
        }
        DisputeResolution::ToBuyer => {
            tracing::info!(
                "Dispute resolved to buyer for order {} - seller's frontend should cancel invoice",
                order_id.0
            );
        }
    }

    state.resolve_dispute(order_id, resolution);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "resolved",
            "resolution": req.resolution,
            "preimage": preimage_hex
        })),
    )
}

// ============ System handlers ============

pub async fn tick(State(state): State<AppState>, Json(req): Json<TickRequest>) -> impl IntoResponse {
    state.advance_time(req.seconds);

    // Process expired orders (auto-confirm shipped orders)
    let expired_orders = state.process_expired_orders();

    // No Fiber RPC calls — seller's frontend will see completed status
    // and call settle_invoice using the preimage from order details.
    for order_id in &expired_orders {
        tracing::info!("Order {} expired and auto-completed, awaiting seller settlement", order_id.0);
    }

    let expired: Vec<Uuid> = expired_orders.iter().map(|id| id.0).collect();
    Json(serde_json::json!(TickResponse { expired_orders: expired }))
}

// ============ Config handler ============

/// Returns Fiber RPC URLs so the frontend knows where to send Fiber calls
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "seller_fiber_rpc_url": state.seller_fiber_rpc_url(),
        "buyer_fiber_rpc_url": state.buyer_fiber_rpc_url()
    }))
}
