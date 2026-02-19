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
}

impl User {
    pub fn new(username: String) -> Self {
        Self {
            id: UserId::new(),
            username,
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
    pub price_sat: u64,
    pub status: ProductStatus,
    pub created_at: DateTime<Utc>,
}

impl Product {
    pub fn new(seller_id: UserId, title: String, description: String, price_sat: u64) -> Self {
        Self {
            id: ProductId::new(),
            seller_id,
            title,
            description,
            price_sat,
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
    pub amount_sat: u64,

    // Payment hash provided by buyer (hash of buyer's preimage)
    pub payment_hash: PaymentHash,
    /// Hold invoice string from Fiber RPC
    pub invoice_string: Option<String>,
    /// Preimage revealed by buyer when confirming receipt
    #[serde(skip_serializing)]
    pub revealed_preimage: Option<Preimage>,

    // State
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,

    // Dispute
    pub dispute: Option<Dispute>,
}

impl Order {
    /// Create a new order with buyer-provided payment_hash
    pub fn new(
        product: &Product,
        buyer_id: UserId,
        payment_hash: PaymentHash,
        timeout_hours: i64,
    ) -> Self {
        Self {
            id: OrderId::new(),
            product_id: product.id,
            product_title: product.title.clone(),
            seller_id: product.seller_id,
            buyer_id,
            amount_sat: product.price_sat,
            payment_hash,
            invoice_string: None,
            revealed_preimage: None,
            status: OrderStatus::WaitingPayment,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(timeout_hours),
            dispute: None,
        }
    }
}
