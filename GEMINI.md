# Fiber Demo: Project Context

This repository contains demo applications showcasing the capabilities of the [Fiber Network](https://fiber.nervos.org/), a Lightning Network-like off-chain payment protocol for the Nervos Network (CKB).

## Project Overview

The project is a Rust-based monorepo consisting of shared libraries and specialized demo services for decentralized games and escrow trading. Both demos follow a **decentralized frontend-driven architecture** — backends manage state only (zero Fiber RPC calls), frontends call Fiber nodes directly from the browser.

### Core Technologies
- **Language:** Rust (Edition 2021)
- **Blockchain:** Nervos Network (CKB)
- **Payment Protocol:** Fiber Network
- **Key Concepts:** Hold Invoices, Preimages, Payment Hashes, Adaptor Signatures.
- **Web Stack:** Axum (Backend), Vanilla HTML/JS (Frontend).
- **Architecture:** Frontend-driven — all Fiber RPC calls made from browser JavaScript, backends have zero Fiber node dependencies.

## Project Structure

The codebase is organized into three main workspaces:

1.  **`fiber-core/`**: Shared library providing cryptographic primitives and Fiber Network client abstractions.
    - `crypto/`: Implementation of `Preimage` and `PaymentHash`.
    - `fiber/`: `FiberClient` trait with `MockFiberClient` (in-memory) and `RpcFiberClient` (for real nodes).
2.  **`fiber-game/`**: A decentralized two-player game protocol.
    - `fiber-game-core/`: Protocol logic using Adaptor Signatures to bind game outcomes to payment releases.
    - `fiber-game-oracle/`: Minimal-trust service that signs game results.
    - `fiber-game-player/`: Service representing a player in the game.
    - `fiber-game-demo/`: Combined demo runner (Oracle + 2 Players on single port).
    - Backend makes **zero** Fiber RPC calls. Frontend JavaScript calls each player's Fiber node directly.
3.  **`fiber-escrow/`**: An escrow trading system for goods and services.
    - `fiber-escrow-service/`: A single service managing products, orders, and disputes.
    - Backend makes **zero** Fiber RPC calls. Seller's browser creates invoices, buyer's browser sends payments, seller's browser settles invoices.
    - Escrow holds preimage and reveals it via API when order completes.
    - `/api/config` endpoint returns Fiber RPC URLs to frontend.

## Building and Running

### Build All
```bash
# Build each workspace independently
cd fiber-core && cargo build && cd ..
cd fiber-game && cargo build && cd ..
cd fiber-escrow && cargo build && cd ..
```

### Running Tests
```bash
# Core primitives and mock client tests
cd fiber-core && cargo test

# Game protocol and E2E flow tests
cd fiber-game && cargo test

# Escrow E2E tests
cd fiber-escrow && cargo test
```

### Running Services
Each demo runs as a web service. Fiber RPC URLs are passed as environment variables and forwarded to the frontend — the backends never call Fiber nodes directly.

- **Game Demo:** `http://localhost:3000`
  ```bash
  cd fiber-game/crates/fiber-game-demo && cargo run
  # With real Fiber nodes:
  FIBER_PLAYER_A_RPC_URL=http://localhost:8227 \
  FIBER_PLAYER_B_RPC_URL=http://localhost:8229 \
  cargo run
  ```

- **Escrow Demo:** `http://localhost:3000`
  ```bash
  cd fiber-escrow/crates/fiber-escrow-service && cargo run
  # With real Fiber nodes:
  FIBER_SELLER_RPC_URL=http://localhost:8227 \
  FIBER_BUYER_RPC_URL=http://localhost:8229 \
  cargo run
  ```

## Development Conventions

- **Surgical Updates:** When modifying protocol logic, ensure that changes are reflected across the `MockFiberClient` to maintain testability.
- **Testing:** New features or protocol changes MUST include E2E tests (see `fiber-game/crates/fiber-game-core/tests/` for examples).
- **Documentation:** Architectural decisions are tracked in `docs/plans/`. Refer to these before making structural changes.
- **Formatting:** Adhere to standard Rust formatting (`cargo fmt`).
- **Frontend-driven Fiber:** All Fiber RPC calls happen in browser JavaScript. Backends must not import or use `RpcFiberClient` — they only store/pass Fiber RPC URLs as strings.

## Testing with Real Nodes
For testing against real Fiber nodes, use the provided setup script:
```bash
./scripts/setup-fiber-testnet.sh
```
This script automates the download of `fnn` (Fiber Node) and `ckb-cli`, sets up two local nodes, funds them via a faucet, and establishes a payment channel between them.

## Important Files
- `README.md`: High-level project summary.
- `fiber-core/src/fiber/traits.rs`: Defines the `FiberClient` interface used by `fiber-core`.
- `docs/plans/2026-02-16-decentralized-preimage-game-design.md`: Detailed game protocol design.
- `docs/plans/2026-02-17-fiber-escrow-design.md`: Detailed escrow system design.
