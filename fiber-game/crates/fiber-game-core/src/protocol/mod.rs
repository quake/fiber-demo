//! Protocol types and messages.

mod messages;
mod types;

pub use messages::{
    CommitMessage, EncryptedPreimageExchange, HoldInvoiceMessage, OracleResultMessage,
    RevealMessage,
};
pub use types::{GameId, GameResult, GameSession, Player};
