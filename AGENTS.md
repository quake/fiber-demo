# AGENTS.md - Coding Agent Guidelines

This document provides guidelines for AI coding agents working in this repository.

## Project Overview

This is a multi-workspace Rust project demonstrating Fiber Network applications:

- `fiber-core/` - Shared library (crypto primitives, FiberClient trait, MockFiberClient)
- `fiber-game/` - Two-player game protocol demo (Rock-Paper-Scissors, Guess Number)
- `fiber-escrow/` - Escrow trading system demo (hold invoice based)

## Build Commands

### Build All Projects
```bash
cd fiber-core && cargo build
cd fiber-game && cargo build
cd fiber-escrow && cargo build
```

### Run Tests
```bash
# All tests in a workspace
cd fiber-core && cargo test
cd fiber-game && cargo test
cd fiber-escrow && cargo test

# Single test by name
cd fiber-core && cargo test test_preimage_hash_roundtrip
cd fiber-game && cargo test test_full_rps_game_a_wins

# Tests in a specific crate
cd fiber-game && cargo test -p fiber-game-core

# Tests with output
cargo test -- --nocapture

# End-to-end HTTP service tests (starts real services)
cd fiber-game && cargo test --test e2e_game_flow -- --nocapture --test-threads=1
cd fiber-escrow && cargo test --test e2e_escrow_flow -- --nocapture --test-threads=1
```

### Run Services

**Note:** Services with Web UI must be started from their crate directory for static files to load correctly. All services support the `PORT` environment variable.

```bash
# Escrow demo (http://localhost:3000)
cd fiber-escrow/crates/fiber-escrow-service && cargo run

# With custom port
cd fiber-escrow/crates/fiber-escrow-service && PORT=3001 cargo run

# Game demo - Combined (Recommended, single command)
cd fiber-game/crates/fiber-game-demo && cargo run
# Opens:
#   Player A: http://localhost:3000/player-a/
#   Player B: http://localhost:3000/player-b/

# Game demo - Separate services (legacy)
cd fiber-game/crates/fiber-game-oracle && cargo run -p fiber-game-oracle
cd fiber-game/crates/fiber-game-player && cargo run -p fiber-game-player
```

### Two-Player Local Testing

**Recommended: Use the combined demo service**

```bash
cd fiber-game/crates/fiber-game-demo && cargo run
```

Then open two browser windows:
- Player A: http://localhost:3000/player-a/
- Player B: http://localhost:3000/player-b/

**Alternative: Separate services (requires 3 terminals)**

```bash
# Terminal 1 - Oracle (must start first)
cd fiber-game/crates/fiber-game-oracle && cargo run

# Terminal 2 - Player A (http://localhost:3001)
cd fiber-game/crates/fiber-game-player && cargo run

# Terminal 3 - Player B (http://localhost:3002)
cd fiber-game/crates/fiber-game-player && PORT=3002 ORACLE_URL=http://localhost:3000 cargo run
```

### Linting
```bash
cargo clippy --all-targets --all-features
cargo fmt --check
```

### Fiber Testnet Setup

The `scripts/setup-fiber-testnet.sh` script automates setting up two local Fiber nodes connected to the testnet:

```bash
# First run - downloads binaries, creates accounts, starts nodes, opens channels
./scripts/setup-fiber-testnet.sh

# Check channel status
./scripts/setup-fiber-testnet.sh status

# Stop running nodes
./scripts/setup-fiber-testnet.sh stop
```

**What the script does:**
1. Downloads `fnn` (Fiber Node) v0.7.0 and `ckb-cli` v2.0.0
2. Creates two CKB accounts using ckb-cli
3. Displays addresses for funding via [CKB Faucet](https://faucet.nervos.org)
4. Auto-checks balances and waits for funding (checks every 3 seconds)
5. Starts two local Fiber nodes (NodeA on port 8227, NodeB on port 8229)
6. Connects NodeA to NodeB directly
7. Opens a 500 CKB channel between the two local nodes

**Requirements:**
- `curl`, `tar`, `unzip` (for binary downloads)
- `jq` (for status command)
- ~1000 CKB total from faucet (only NodeA needs funds to open the channel)

**Directory structure created:**
```
testnet-fnn/
├── bin/
│   ├── fnn
│   └── ckb-cli
├── nodeA/
│   ├── ckb/
│   │   ├── key          # Private key
│   │   ├── address      # CKB testnet address
│   │   └── lock_arg
│   ├── config.yml
│   ├── fnn.log
│   └── fnn.pid
└── nodeB/
    └── (same structure)
```

## Code Style Guidelines

### Import Organization
Order imports in groups separated by blank lines:
1. Standard library (`std::`)
2. External crates (alphabetical)
3. Internal crate modules (`crate::`, `super::`)

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::*;
use crate::state::AppState;
```

### Module Documentation
Every module file should start with a `//!` doc comment:
```rust
//! Cryptographic primitives for Fiber Network.
//! 
//! This module provides Preimage and PaymentHash types.
```

### Type Definitions

**Newtype Pattern** - Use tuple structs for domain IDs:
```rust
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
```

**Enums** - Use `#[serde(rename_all = "snake_case")]` for API enums:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    WaitingPayment,
    Funded,
    Shipped,
}
```

### Error Handling

Use `thiserror` for error types:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FiberError {
    #[error("Invoice not found: {0}")]
    InvoiceNotFound(PaymentHash),

    #[error("Invalid preimage: does not match payment hash")]
    InvalidPreimage,
}
```

### Async Traits

Use `#[allow(async_fn_in_trait)]` for async trait methods:
```rust
#[allow(async_fn_in_trait)]
pub trait FiberClient: Send + Sync {
    async fn create_hold_invoice(...) -> Result<HoldInvoice, FiberError>;
}
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types, Traits | PascalCase | `PaymentHash`, `FiberClient` |
| Functions, Methods | snake_case | `create_hold_invoice`, `payment_hash` |
| Constants | SCREAMING_SNAKE_CASE | `DEFAULT_EXPIRY_SECS` |
| Module files | snake_case | `payment.rs`, `hold_invoice.rs` |
| Test functions | `test_` prefix | `test_preimage_hash_roundtrip` |

### Test Structure

Place unit tests in the same file with `#[cfg(test)]` module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preimage_hash_roundtrip() {
        let preimage = Preimage::random();
        let hash = preimage.payment_hash();
        assert!(hash.verify(&preimage));
    }
}
```

Place integration tests in `tests/` directory:
```rust
// tests/full_game_flow.rs
#[tokio::test]
async fn test_full_rps_game_a_wins() {
    // Test implementation
}
```

### Struct Documentation

Document public items with `///` comments:
```rust
/// 32-byte preimage, its hash is the payment_hash
#[derive(Clone, Serialize, Deserialize)]
pub struct Preimage([u8; 32]);

impl Preimage {
    /// Create a new random preimage
    pub fn random() -> Self { ... }

    /// Compute the payment hash (SHA256 of preimage)
    pub fn payment_hash(&self) -> PaymentHash { ... }
}
```

### Workspace Dependencies

Define shared dependencies in workspace `Cargo.toml`:
```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

Reference in crate `Cargo.toml`:
```toml
[dependencies]
serde = { workspace = true }
tokio = { workspace = true }
```

### HTTP Handlers (Axum)

Return `impl IntoResponse` for flexible response types:
```rust
pub async fn create_order(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateOrderRequest>,
) -> impl IntoResponse {
    // Validation
    if condition {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "message"})));
    }
    // Success
    (StatusCode::OK, Json(json!({"order_id": id})))
}
```

## Project-Specific Notes

### fiber-core
- Contains `Preimage`, `PaymentHash` - core crypto types
- `FiberClient` trait abstracts Fiber Network operations
- `MockFiberClient` for testing with simulated balances

### fiber-game
- Uses secp256k1 for signature-based game resolution
- Oracle generates adaptor signatures for game outcomes
- Four crates: core library, oracle service, player service, combined demo
- Combined demo (`fiber-game-demo`) runs Oracle + 2 Players on single port

### fiber-escrow
- Single service with multi-role Web UI
- Pre-registered demo users: alice, bob, carol
- Time simulation via `/api/system/tick` for testing timeouts
