# Fiber Demo

Demo applications showcasing [Fiber Network](https://fiber.nervos.org/) payment channel capabilities.

## Projects

| Project | Description | Status |
|---------|-------------|--------|
| [fiber-core](./fiber-core/) | Shared library with crypto primitives and FiberClient trait | Complete |
| [fiber-game](./fiber-game/) | Decentralized two-player game protocol (Rock-Paper-Scissors, Guess Number) | Complete |
| [fiber-escrow](./fiber-escrow/) | Escrow trading system with hold invoice-based payment | Complete |

## Architecture

```
fiber-demo/
├── fiber-core/                    # Shared library
│   └── src/
│       ├── crypto/                # Preimage, PaymentHash
│       └── fiber/                 # FiberClient trait, MockFiberClient
├── fiber-game/                    # Game demo
│   └── crates/
│       ├── fiber-game-core/       # Game protocol logic
│       ├── fiber-game-oracle/     # Oracle service (adaptor signatures)
│       └── fiber-game-player/     # Player service
└── fiber-escrow/                  # Escrow demo
    └── crates/
        └── fiber-escrow-service/  # Single service with multi-role UI
```

## Quick Start

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))

### Build All

```bash
# Build each workspace
cd fiber-core && cargo build && cd ..
cd fiber-game && cargo build && cd ..
cd fiber-escrow && cargo build && cd ..
```

### Run Tests

```bash
# fiber-core tests
cd fiber-core && cargo test

# fiber-game tests
cd fiber-game && cargo test
```

### Run Services

```bash
# Escrow demo (http://localhost:3000)
cd fiber-escrow && cargo run

# Game demo - Oracle (http://localhost:3001)
cd fiber-game && cargo run -p fiber-game-oracle

# Game demo - Player (http://localhost:3002)
cd fiber-game && cargo run -p fiber-game-player
```

## Key Concepts

### Hold Invoices

Both demos use Fiber Network's hold invoice mechanism:

1. **Payer locks funds** by paying a hold invoice (knows payment_hash, not preimage)
2. **Funds remain locked** until preimage is revealed or invoice expires
3. **Payee claims funds** by revealing the preimage
4. **Or funds return** if the invoice is cancelled

### fiber-core Types

- `Preimage` - 32-byte secret, revealed to claim payment
- `PaymentHash` - SHA256 of preimage, used to create invoices
- `FiberClient` - Trait abstracting Fiber Network operations
- `MockFiberClient` - In-memory implementation for testing/demos

## Documentation

- [Escrow Design](./docs/plans/2026-02-17-fiber-escrow-design.md)
- [Escrow Implementation Plan](./docs/plans/2026-02-17-fiber-escrow-implementation.md)

## License

MIT
