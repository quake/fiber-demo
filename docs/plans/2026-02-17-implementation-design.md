# Fiber Game Implementation Design

## Overview

Implementation of the decentralized two-player game protocol using Rust. This phase focuses on the complete preimage acquisition flow with mock Fiber integration.

## Key Decisions

| Decision | Choice |
|----------|--------|
| Language | Rust |
| Project structure | Library + Player HTTP service + Oracle HTTP service |
| Oracle communication | HTTP API |
| Player communication | All via Oracle (central hub + lobby) |
| Player interaction | Web UI |
| Fiber integration | Trait abstraction with mock implementation |
| Crypto primitives | Existing crates (secp256k1, sha2) |
| Initial game types | Rock-Paper-Scissors + Guess the Number |
| Testing | Unit + Integration tests |

---

## Project Structure

```
fiber-game/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── fiber-game-core/       # Library: protocol logic, crypto, game types
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── crypto/        # Adaptor signatures, commitments
│   │   │   ├── protocol/      # Protocol phases, messages
│   │   │   ├── games/         # Game definitions (RPS, Guess Number)
│   │   │   └── fiber/         # FiberClient trait + MockFiberClient
│   │   └── Cargo.toml
│   ├── fiber-game-oracle/     # Binary: Oracle HTTP service
│   │   ├── src/
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   └── fiber-game-player/     # Binary: Player HTTP service + Web UI
│       ├── src/
│       │   └── main.rs
│       ├── static/            # HTML/JS/CSS for web UI
│       └── Cargo.toml
└── tests/                     # Integration tests
    └── full_game_flow.rs
```

**Dependencies:**
- `secp256k1` - Schnorr signatures, key generation
- `sha2` - Hashing (SHA256)
- `axum` - HTTP server (Oracle + Player)
- `reqwest` - HTTP client for Oracle communication
- `serde` / `serde_json` - Message serialization
- `tokio` - Async runtime
- `uuid` - Game ID generation
- `tower-http` - Static file serving for web UI

---

## Core Library - Crypto Module

```rust
// crates/fiber-game-core/src/crypto/mod.rs

/// 32-byte preimage, its hash is the payment_hash
pub struct Preimage([u8; 32]);

/// SHA256 hash of preimage
pub struct PaymentHash([u8; 32]);

/// Commitment = H(action || salt)
pub struct Commitment([u8; 32]);

/// Salt for commitment scheme
pub struct Salt([u8; 32]);

/// Oracle's signature on game result
pub struct OracleSignature {
    pub signature: schnorr::Signature,
    pub message: Vec<u8>,
}

/// Signature point for adaptor signatures
/// sig_point = R + H(R || O || game_id || result) * O
pub struct SignaturePoint(PublicKey);

/// Encrypted preimage = preimage XOR H(sig_point)
pub struct EncryptedPreimage([u8; 32]);

impl EncryptedPreimage {
    /// Encrypt preimage with signature point
    pub fn encrypt(preimage: &Preimage, sig_point: &SignaturePoint) -> Self;
    
    /// Decrypt using Oracle's actual signature
    /// Extract secret from signature, then XOR to recover preimage
    pub fn decrypt(&self, oracle_sig: &OracleSignature) -> Result<Preimage, Error>;
}

/// Compute signature points for all possible outcomes
pub fn compute_signature_points(
    oracle_pubkey: &PublicKey,
    commitment_point: &PublicKey,  // R
    game_id: &GameId,
) -> SignaturePoints {
    SignaturePoints {
        a_wins: /* R + H(R || O || game_id || "A wins") * O */,
        b_wins: /* R + H(R || O || game_id || "B wins") * O */,
        draw:   /* R + H(R || O || game_id || "Draw") * O */,
    }
}
```

---

## Core Library - Protocol Types & Messages

```rust
// crates/fiber-game-core/src/protocol/types.rs

pub struct GameId(pub Uuid);

impl GameId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

pub enum GameResult {
    AWins,
    BWins,
    Draw,
}

pub enum Player {
    A,
    B,
}

pub struct GameSession {
    pub game_id: GameId,
    pub game_type: GameType,
    pub oracle_pubkey: PublicKey,
    pub oracle_commitment_point: PublicKey,  // R
    pub oracle_commitment: Option<[u8; 32]>, // For games with Oracle secret
}

// crates/fiber-game-core/src/protocol/messages.rs

/// Phase 2: Hold invoice exchange
pub struct HoldInvoice {
    pub payment_hash: PaymentHash,
    pub amount_sat: u64,
    pub expiry_secs: u64,
}

/// Phase 3: Encrypted preimage exchange
pub struct EncryptedPreimageExchange {
    pub game_id: GameId,
    pub player: Player,
    pub encrypted_preimage: EncryptedPreimage,
}

/// Phase 4: Commitment
pub struct CommitMessage {
    pub game_id: GameId,
    pub player: Player,
    pub commitment: Commitment,
}

/// Phase 5: Reveal to Oracle
pub struct RevealMessage {
    pub game_id: GameId,
    pub player: Player,
    pub action: GameAction,  // Game-specific
    pub salt: Salt,
    pub commit_a: Commitment,
    pub commit_b: Commitment,
}

/// Phase 6: Oracle's signed result
pub struct OracleResultMessage {
    pub game_id: GameId,
    pub game_type: GameType,
    pub game_data: GameData,  // All inputs needed to verify
    pub result: GameResult,
    pub signature: OracleSignature,
}
```

---

## Core Library - Game Definitions

```rust
// crates/fiber-game-core/src/games/mod.rs

pub enum GameType {
    RockPaperScissors,
    GuessNumber,
}

/// Game-specific action
pub enum GameAction {
    Rps(RpsAction),
    GuessNumber(u8),  // 0-99
}

pub enum RpsAction {
    Rock,
    Paper,
    Scissors,
}

/// Trait for game logic - each game type implements this
pub trait GameJudge {
    /// Determine winner from actions and optional Oracle secret
    fn judge(
        action_a: &GameAction,
        action_b: &GameAction,
        oracle_secret: Option<&OracleSecret>,
    ) -> GameResult;

    /// Validate that an action is legal for this game
    fn validate_action(action: &GameAction) -> bool;

    /// Does this game require Oracle to commit a secret beforehand?
    fn requires_oracle_secret() -> bool;
}

// crates/fiber-game-core/src/games/rps.rs
impl GameJudge for RpsGame {
    fn judge(action_a: &GameAction, action_b: &GameAction, _: Option<&OracleSecret>) -> GameResult {
        // Rock beats Scissors, Scissors beats Paper, Paper beats Rock
    }
    
    fn requires_oracle_secret() -> bool { false }
}

// crates/fiber-game-core/src/games/guess_number.rs
pub struct OracleSecret {
    pub secret_number: u8,  // 0-99
    pub nonce: [u8; 32],
}

impl GameJudge for GuessNumberGame {
    fn judge(action_a: &GameAction, action_b: &GameAction, oracle_secret: Option<&OracleSecret>) -> GameResult {
        // Player closer to secret_number wins
    }
    
    fn requires_oracle_secret() -> bool { true }
}
```

---

## Core Library - Fiber Client Trait

```rust
// crates/fiber-game-core/src/fiber/mod.rs

pub trait FiberClient {
    /// Create a hold invoice that locks funds until settled or cancelled
    async fn create_hold_invoice(
        &self,
        payment_hash: &PaymentHash,
        amount_sat: u64,
        expiry_secs: u64,
    ) -> Result<HoldInvoice, FiberError>;

    /// Pay a hold invoice (funds locked on our side)
    async fn pay_hold_invoice(
        &self,
        invoice: &HoldInvoice,
    ) -> Result<PaymentId, FiberError>;

    /// Settle a received hold invoice with preimage (claim funds)
    async fn settle_invoice(
        &self,
        payment_hash: &PaymentHash,
        preimage: &Preimage,
    ) -> Result<(), FiberError>;

    /// Cancel a hold invoice (refund locked funds)
    async fn cancel_invoice(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<(), FiberError>;

    /// Check payment status
    async fn get_payment_status(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<PaymentStatus, FiberError>;
}

pub enum PaymentStatus {
    Pending,   // Hold invoice created, not yet paid
    Held,      // Funds locked, waiting for preimage
    Settled,   // Completed with preimage
    Cancelled, // Refunded
}

// crates/fiber-game-core/src/fiber/mock.rs

/// In-memory mock for testing
pub struct MockFiberClient {
    invoices: HashMap<PaymentHash, MockInvoiceState>,
}

impl FiberClient for MockFiberClient {
    // Track state transitions in memory
    // Validate correct preimage on settle
}
```

---

## Oracle HTTP Service

```rust
// crates/fiber-game-oracle/src/main.rs

/// Oracle state
struct OracleState {
    keypair: Keypair,
    games: HashMap<GameId, GameState>,
}

struct GameState {
    game_type: GameType,
    amount_sat: u64,
    status: GameStatus,
    commitment_point: PublicKey,         // R for this game
    oracle_secret: Option<OracleSecret>, // For GuessNumber
    oracle_commitment: Option<[u8; 32]>, // H(secret || nonce)
    player_a_id: Uuid,
    player_b_id: Option<Uuid>,
    invoice_a: Option<HoldInvoice>,
    invoice_b: Option<HoldInvoice>,
    encrypted_preimage_a: Option<EncryptedPreimage>,
    encrypted_preimage_b: Option<EncryptedPreimage>,
    commit_a: Option<Commitment>,
    commit_b: Option<Commitment>,
    reveal_a: Option<RevealMessage>,
    reveal_b: Option<RevealMessage>,
    result: Option<OracleResultMessage>,
    reveal_deadline: Option<Instant>,
}

enum GameStatus {
    WaitingForOpponent,  // Listed in /games/available
    InProgress,          // Player B joined
    Completed,           // Result signed
    Cancelled,           // Timeout with no join
}
```

### Oracle HTTP Endpoints

```
GET  /oracle/pubkey
     Response: { pubkey }

GET  /games/available
     Query: ?game_type=rps (optional filter)
     Response: { games: [{ game_id, game_type, amount_sat, created_at }] }

POST /game/create
     Body: { game_type, player_a_id, amount_sat, timeout_secs }
     Response: { game_id, oracle_pubkey, commitment_point, oracle_commitment? }

POST /game/{game_id}/join
     Body: { player_b_id }
     Response: { status, oracle_pubkey, commitment_point, oracle_commitment? }

POST /game/{game_id}/invoice
     Body: { player, payment_hash, amount_sat }
     Response: { status }

GET  /game/{game_id}/invoice/{opponent}
     Response: { payment_hash, amount_sat }

POST /game/{game_id}/encrypted-preimage
     Body: { player, encrypted_preimage }
     Response: { status }

GET  /game/{game_id}/encrypted-preimage/{opponent}
     Response: { encrypted_preimage }

POST /game/{game_id}/commit
     Body: { player, commitment }
     Response: { status }

POST /game/{game_id}/reveal
     Body: { player, action, salt, commit_a, commit_b }
     Response: { status: "accepted" | "waiting" }

GET  /game/{game_id}/result
     Response: { status, result?, signature?, game_data? }
```

---

## Player HTTP Service

```rust
// crates/fiber-game-player/src/main.rs

struct PlayerState {
    player_id: Uuid,
    keypair: Keypair,
    fiber_client: Box<dyn FiberClient>,
    oracle_url: String,
    games: HashMap<GameId, PlayerGameState>,
}

struct PlayerGameState {
    role: Player,  // A or B
    preimage: Preimage,
    payment_hash: PaymentHash,
    salt: Salt,
    action: Option<GameAction>,
    opponent_encrypted_preimage: Option<EncryptedPreimage>,
    phase: PlayerGamePhase,
}

enum PlayerGamePhase {
    WaitingForOpponent,
    ExchangingInvoices,
    ExchangingEncryptedPreimages,
    WaitingForAction,
    Committed,
    Revealed,
    WaitingForResult,
    Settled,
}
```

### Player HTTP Endpoints

```
GET  /
     Response: HTML page with game UI

GET  /api/games/available
     Response: { games: [...] }  (proxies to Oracle)

GET  /api/games/mine
     Response: { games: [...] }

POST /api/game/create
     Body: { game_type, amount_sat }
     Response: { game_id }

POST /api/game/join
     Body: { game_id }
     Response: { status }

POST /api/game/{game_id}/play
     Body: { action }
     Response: { status }

GET  /api/game/{game_id}/status
     Response: { phase, result?, ... }

POST /api/game/{game_id}/settle
     Response: { result, amount_won }
```

---

## Web UI Pages

```
1. Home (/)
   ┌─────────────────────────────────────────────┐
   │  Fiber Game Demo                            │
   │                                             │
   │  [Create New Game]                          │
   │                                             │
   │  ─── Available Games ───                    │
   │  | Type | Amount | Created   | Action     | │
   │  | RPS  | 1000   | 2 min ago | [Join]     | │
   │  | Guess| 500    | 5 min ago | [Join]     | │
   │                                             │
   │  ─── My Games ───                           │
   │  | Game ID | Type | Status      | Action | │
   │  | abc-123 | RPS  | Your turn   | [Play] | │
   │  | def-456 | Guess| Waiting...  | [View] | │
   └─────────────────────────────────────────────┘

2. Create Game (/create)
   - Select game type (RPS / Guess Number)
   - Enter amount (sats)
   - Click "Create" → redirects to game page

3. Game Page (/game/{game_id})
   ┌─────────────────────────────────────────────┐
   │  Game: abc-123 (Rock-Paper-Scissors)        │
   │  Amount: 1000 sats                          │
   │  Status: Waiting for your move              │
   │                                             │
   │  ┌─────┐  ┌─────┐  ┌─────────┐             │
   │  │Rock │  │Paper│  │Scissors │             │
   │  └─────┘  └─────┘  └─────────┘             │
   │                                             │
   │  [Submit Move]                              │
   │                                             │
   │  ─── Progress ───                           │
   │  ✓ Invoice exchanged                        │
   │  ✓ Encrypted preimages exchanged            │
   │  ✓ Committed                                │
   │  ○ Waiting for opponent reveal...           │
   └─────────────────────────────────────────────┘

4. Result Page (/game/{game_id}/result)
   ┌─────────────────────────────────────────────┐
   │  Game Complete!                             │
   │                                             │
   │  You: Rock    Opponent: Scissors            │
   │                                             │
   │  You Win! +1000 sats                        │
   │                                             │
   │  [Settle Now]  (claims funds)               │
   │                                             │
   │  Settlement status: Pending / Complete      │
   └─────────────────────────────────────────────┘
```

---

## Testing Strategy

### Unit Tests

```rust
// crates/fiber-game-core/src/...

#[cfg(test)]
mod tests {
    // Crypto module tests
    - test_preimage_hash_roundtrip()
    - test_commitment_verification()
    - test_encrypted_preimage_encrypt_decrypt()
    - test_signature_point_computation()

    // Game logic tests
    - test_rps_all_outcomes()          // 9 combinations
    - test_rps_validate_action()
    - test_guess_number_closer_wins()
    - test_guess_number_tie()
    - test_guess_number_oracle_commitment_verification()

    // Mock Fiber client tests
    - test_hold_invoice_lifecycle()
    - test_settle_with_correct_preimage()
    - test_settle_with_wrong_preimage_fails()
    - test_cancel_invoice()
}
```

### Integration Tests

```rust
// tests/full_game_flow.rs

#[tokio::test]
async fn test_full_rps_game_a_wins() {
    // 1. Start Oracle server
    // 2. Start Player A server
    // 3. Start Player B server
    // 4. A creates game
    // 5. B joins game
    // 6. Both exchange invoices + encrypted preimages
    // 7. A plays Rock, B plays Scissors
    // 8. Both reveal to Oracle
    // 9. Oracle signs "A wins"
    // 10. A decrypts B's preimage
    // 11. A settles invoice
    // 12. Assert: A's balance increased, B's decreased
}

#[tokio::test]
async fn test_full_rps_game_draw() { ... }

#[tokio::test]
async fn test_guess_number_b_wins() { ... }

#[tokio::test]
async fn test_timeout_results_in_draw() { ... }

#[tokio::test]
async fn test_invalid_reveal_rejected() { ... }
```

---

## Protocol Flow Diagram

```
Player A                    Oracle                      Player B
   │                          │                            │
   │──POST /game/create──────>│                            │
   │<─────{ game_id }─────────│                            │
   │                          │                            │
   │                          │<───GET /games/available────│
   │                          │────{ games: [...] }───────>│
   │                          │                            │
   │                          │<───POST /game/{id}/join────│
   │                          │────{ status: ok }─────────>│
   │                          │                            │
   │──POST /invoice──────────>│<───POST /invoice───────────│
   │                          │                            │
   │──GET /invoice/B─────────>│                            │
   │<────{ invoice_b }────────│                            │
   │                          │<───GET /invoice/A──────────│
   │                          │────{ invoice_a }──────────>│
   │                          │                            │
   │  (both pay opponent's hold invoice via Fiber)         │
   │                          │                            │
   │──POST /encrypted-preimage>│<──POST /encrypted-preimage│
   │──GET /encrypted-preimage/B│                           │
   │<────{ enc_preimage_b }───│                            │
   │                          │<──GET /encrypted-preimage/A│
   │                          │───{ enc_preimage_a }──────>│
   │                          │                            │
   │──POST /commit───────────>│<───POST /commit────────────│
   │                          │                            │
   │──POST /reveal───────────>│<───POST /reveal────────────│
   │                          │                            │
   │                    [Oracle judges]                    │
   │                    [Oracle signs]                     │
   │                          │                            │
   │──GET /result────────────>│<───GET /result─────────────│
   │<────{ A wins, sig }──────│────{ A wins, sig }────────>│
   │                          │                            │
   │  [A decrypts B's preimage using sig]                  │
   │  [A settles B's invoice]                              │
   │                          │                            │
   │                          │      [B cancels A's invoice]
```

---

## Next Steps

1. Initialize Rust workspace with Cargo.toml
2. Implement core crypto module
3. Implement game logic (RPS + Guess Number)
4. Implement mock Fiber client
5. Build Oracle HTTP service
6. Build Player HTTP service + Web UI
7. Write unit tests
8. Write integration tests
