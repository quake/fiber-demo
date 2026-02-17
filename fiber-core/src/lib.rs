//! Fiber Core Library
//!
//! Shared primitives for Fiber Network applications:
//! - Cryptographic primitives (Preimage, PaymentHash)
//! - FiberClient trait and MockFiberClient

pub mod crypto;
pub mod fiber;

pub use crypto::{PaymentHash, Preimage};
pub use fiber::{FiberClient, FiberError, HoldInvoice, MockFiberClient, PaymentId, PaymentStatus};
