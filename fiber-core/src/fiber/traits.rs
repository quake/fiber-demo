//! Fiber client trait definition.

use crate::crypto::{PaymentHash, Preimage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Errors from Fiber operations
#[derive(Debug, Error)]
pub enum FiberError {
    #[error("Invoice not found: {0}")]
    InvoiceNotFound(PaymentHash),

    #[error("Invalid preimage: does not match payment hash")]
    InvalidPreimage,

    #[error("Invoice already settled")]
    AlreadySettled,

    #[error("Invoice already cancelled")]
    AlreadyCancelled,

    #[error("Invoice expired")]
    Expired,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Hold invoice information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoldInvoice {
    /// Payment hash (derived from preimage)
    pub payment_hash: PaymentHash,
    /// Amount in shannons
    pub amount: u64,
    /// Expiry time in seconds
    pub expiry_secs: u64,
    /// Invoice string (bolt11 or similar)
    pub invoice_string: String,
}

/// Payment identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaymentId(Uuid);

impl PaymentId {
    /// Create a new random payment ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PaymentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Payment status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    /// Hold invoice created, not yet paid
    Pending,
    /// Funds locked, waiting for preimage
    Held,
    /// Completed with preimage
    Settled,
    /// Refunded
    Cancelled,
}

/// Trait for Fiber Network client operations
///
/// This trait abstracts the Fiber Network operations needed for the game protocol.
/// Implementations can be:
/// - MockFiberClient for testing
/// - Real Fiber Network client for production
#[async_trait]
pub trait FiberClient: Send + Sync {
    /// Support downcasting to concrete types
    fn as_any(&self) -> &dyn std::any::Any;

    /// Create a hold invoice that locks funds until settled or cancelled
    async fn create_hold_invoice(
        &self,
        payment_hash: &PaymentHash,
        amount: u64,
        expiry_secs: u64,
    ) -> Result<HoldInvoice, FiberError>;

    /// Pay a hold invoice (funds locked on our side)
    async fn pay_hold_invoice(&self, invoice: &HoldInvoice) -> Result<PaymentId, FiberError>;

    /// Settle a received hold invoice with preimage (claim funds)
    async fn settle_invoice(
        &self,
        payment_hash: &PaymentHash,
        preimage: &Preimage,
    ) -> Result<(), FiberError>;

    /// Cancel a hold invoice (refund locked funds)
    async fn cancel_invoice(&self, payment_hash: &PaymentHash) -> Result<(), FiberError>;

    /// Check payment status
    async fn get_payment_status(&self, payment_hash: &PaymentHash)
        -> Result<PaymentStatus, FiberError>;

    /// Get the total local balance in shannons across all open channels
    async fn get_balance(&self) -> Result<u64, FiberError>;
}
