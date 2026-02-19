//! HTTP API handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use fiber_core::FiberClient;
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
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id.0,
            username: u.username,
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
    pub amount_sat: u64,
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
    let order = state.create_order(&product, buyer_id, payment_hash.clone());

    // Store preimage immediately (escrow holds it for timeout/dispute settlement)
    tracing::info!(
        "Storing preimage for order {}: preimage_hash={}, order_payment_hash={}",
        order.id.0,
        preimage.payment_hash().to_hex(),
        order.payment_hash.to_hex()
    );
    state.set_revealed_preimage(order.id, preimage);

    // If Fiber client is configured, create hold invoice on seller's node
    let invoice_string = if let Some(fiber_client) = state.seller_fiber_client() {
        match fiber_client
            .create_hold_invoice(&payment_hash, order.amount_sat, 24 * 60 * 60) // 24 hour expiry
            .await
        {
            Ok(invoice) => {
                // Store invoice string in order
                state.set_order_invoice(order.id, invoice.invoice_string.clone());
                Some(invoice.invoice_string)
            }
            Err(e) => {
                // TODO: Also need to delete the order from state
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to create hold invoice: {}", e)
                    })),
                );
            }
        }
    } else {
        // No Fiber client - mock mode, no invoice created
        None
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "order_id": order.id.0,
            "payment_hash": order.payment_hash.to_string(),
            "amount_sat": order.amount_sat,
            "expires_at": order.expires_at.to_rfc3339(),
            "invoice_string": invoice_string
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

    // Include preimage for seller if order is completed
    let mut response = serde_json::json!(order_to_response(&order));
    
    if order.seller_id == user_id && order.status == OrderStatus::Completed {
        if let Some(preimage) = state.get_revealed_preimage(order_id) {
            response["preimage"] = serde_json::json!(hex::encode(preimage.as_bytes()));
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

    // If Fiber client is configured, verify payment status on seller's node
    // Buyer should have already paid the invoice from their own wallet
    if let Some(seller_client) = state.seller_fiber_client() {
        // Poll for payment to be held (max 30 seconds, check every 2 seconds)
        // This gives time for the payment to propagate through the network
        let max_attempts = 15;
        let mut confirmed = false;

        for attempt in 0..max_attempts {
            match seller_client.get_payment_status(&order.payment_hash).await {
                Ok(status) => {
                    tracing::debug!(
                        "Payment status check {}/{}: {:?}",
                        attempt + 1,
                        max_attempts,
                        status
                    );
                    match status {
                        fiber_core::PaymentStatus::Held => {
                            confirmed = true;
                            break;
                        }
                        fiber_core::PaymentStatus::Settled => {
                            // Already settled (shouldn't happen at this stage)
                            confirmed = true;
                            break;
                        }
                        fiber_core::PaymentStatus::Cancelled => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": "Payment was cancelled"})),
                            );
                        }
                        fiber_core::PaymentStatus::Pending => {
                            // Still waiting, continue polling
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to check payment status: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        if !confirmed {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Payment not received. Please pay the invoice first, then call this endpoint."
                })),
            );
        }
    }
    // If no Fiber client configured, operate in trust mode (for testing)

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

    // If Fiber client is configured, settle the hold invoice on seller's node
    if let Some(fiber_client) = state.seller_fiber_client() {
        if let Err(e) = fiber_client
            .settle_invoice(&order.payment_hash, &preimage)
            .await
        {
            // Log but don't fail - order is already confirmed locally
            // The settlement can be retried or handled manually
            tracing::error!(
                "Failed to settle invoice for order {}: {}. Manual settlement may be required.",
                order_id.0,
                e
            );
        } else {
            tracing::info!("Invoice settled for order {}", order_id.0);
        }
    }

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

    // Handle Fiber invoice based on resolution
    let mut preimage_hex: Option<String> = None;

    if let Some(fiber_client) = state.seller_fiber_client() {
        match resolution {
            DisputeResolution::ToSeller => {
                // Settle invoice - seller gets paid
                if let Some(preimage) = state.get_revealed_preimage(order_id) {
                    match fiber_client
                        .settle_invoice(&order.payment_hash, &preimage)
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                "Settled invoice for disputed order {} (resolved to seller)",
                                order_id.0
                            );
                            preimage_hex = Some(hex::encode(preimage.as_bytes()));
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to settle invoice for disputed order {}: {}",
                                order_id.0,
                                e
                            );
                            // Continue with resolution even if settlement fails
                            // Manual intervention may be needed
                        }
                    }
                } else {
                    tracing::warn!(
                        "No preimage found for disputed order {} - cannot settle invoice",
                        order_id.0
                    );
                }
            }
            DisputeResolution::ToBuyer => {
                // Cancel invoice - buyer gets refund
                match fiber_client.cancel_invoice(&order.payment_hash).await {
                    Ok(()) => {
                        tracing::info!(
                            "Cancelled invoice for disputed order {} (resolved to buyer)",
                            order_id.0
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to cancel invoice for disputed order {}: {}",
                            order_id.0,
                            e
                        );
                        // Continue with resolution even if cancellation fails
                    }
                }
            }
        }
    } else {
        // No Fiber client - just update state and return preimage if resolving to seller
        if resolution == DisputeResolution::ToSeller {
            if let Some(preimage) = state.get_revealed_preimage(order_id) {
                preimage_hex = Some(hex::encode(preimage.as_bytes()));
            }
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

    // Settle invoices for expired orders that have preimage
    if let Some(fiber_client) = state.seller_fiber_client() {
        for (order_id, payment_hash, preimage_opt) in &expired_orders {
            if let Some(preimage) = preimage_opt {
                match fiber_client.settle_invoice(payment_hash, preimage).await {
                    Ok(()) => {
                        tracing::info!("Settled invoice for expired order {}", order_id.0);
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to settle invoice for expired order {}: {}",
                            order_id.0,
                            e
                        );
                    }
                }
            } else {
                tracing::warn!(
                    "Expired order {} has no preimage, cannot settle invoice",
                    order_id.0
                );
            }
        }
    }

    let expired: Vec<Uuid> = expired_orders.iter().map(|(id, _, _)| id.0).collect();
    Json(serde_json::json!(TickResponse { expired_orders: expired }))
}

// ============ Fiber RPC Proxy handlers ============

#[derive(Deserialize)]
pub struct SendPaymentProxyRequest {
    /// Invoice to pay
    pub invoice: String,
}

/// Proxy send_payment request to buyer's Fiber node
/// Uses FIBER_BUYER_RPC_URL environment variable
pub async fn send_payment_proxy(
    State(state): State<AppState>,
    Json(req): Json<SendPaymentProxyRequest>,
) -> impl IntoResponse {
    // Get buyer RPC URL from server config
    let buyer_rpc_url = match state.buyer_fiber_rpc_url() {
        Some(url) => url.to_string(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Buyer Fiber RPC not configured (set FIBER_BUYER_RPC_URL)"})),
            );
        }
    };

    // Create HTTP client and send RPC request
    let client = reqwest::Client::new();
    
    let rpc_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "send_payment",
        "params": [{"invoice": req.invoice}]
    });

    println!("[send_payment_proxy] Sending to {}: {}", buyer_rpc_url, serde_json::to_string(&rpc_request).unwrap_or_default());

    match client
        .post(&buyer_rpc_url)
        .header("Content-Type", "application/json")
        .json(&rpc_request)
        .send()
        .await
    {
        Ok(response) => {
            let status_code = response.status();
            match response.text().await {
                Ok(text) => {
                    println!("[send_payment_proxy] Raw response (HTTP {}): {}", status_code, text);
                    
                    match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(json) => {
                            // Log the parsed JSON structure
                            if let Some(result) = json.get("result") {
                                println!("[send_payment_proxy] Parsed result: {}", serde_json::to_string_pretty(result).unwrap_or_default());
                                if let Some(status) = result.get("status") {
                                    println!("[send_payment_proxy] Payment status: {}", status);
                                }
                                if let Some(failed_error) = result.get("failed_error") {
                                    println!("[send_payment_proxy] Failed error: {}", failed_error);
                                }
                            }
                            
                            if let Some(error) = json.get("error") {
                                let error_msg = error
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("RPC error");
                                (
                                    StatusCode::BAD_REQUEST,
                                    Json(serde_json::json!({"error": error_msg, "details": error})),
                                )
                            } else if let Some(result) = json.get("result") {
                                // Return the full result, frontend will check status
                                (StatusCode::OK, Json(result.clone()))
                            } else {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"error": "Invalid RPC response", "raw": text})),
                                )
                            }
                        }
                        Err(e) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": format!("Failed to parse JSON: {}", e), "raw": text})),
                        ),
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Failed to read response: {}", e)})),
                ),
            }
        }
        Err(e) => {
            println!("[send_payment_proxy] Connection error: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("Failed to connect to Fiber node: {}", e)})),
            )
        }
    }
}
