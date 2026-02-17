//! Game definitions and logic.

mod guess_number;
mod rps;
mod traits;

pub use guess_number::{GuessNumberGame, OracleSecret};
pub use rps::{RpsAction, RpsGame};
pub use traits::{GameAction, GameJudge, GameType};
