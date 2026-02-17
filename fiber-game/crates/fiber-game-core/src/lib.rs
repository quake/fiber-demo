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
