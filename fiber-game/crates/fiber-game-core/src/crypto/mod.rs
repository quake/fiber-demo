//! Cryptographic primitives for the Fiber Game protocol.
//!
//! This module provides:
//! - Preimage and PaymentHash for hold invoices
//! - Commitment and Salt for commit-reveal scheme
//! - SignaturePoint and EncryptedPreimage for adaptor signatures

mod commitment;
mod encrypted_preimage;
mod payment;
mod signature_point;

pub use commitment::{Commitment, Salt};
pub use encrypted_preimage::EncryptedPreimage;
pub use payment::{PaymentHash, Preimage};
pub use signature_point::{compute_signature_points, SignaturePoint, SignaturePoints};
