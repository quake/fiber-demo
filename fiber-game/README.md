# Fiber Game

A decentralized two-player game protocol built on Fiber Network, using adaptor signatures and hold invoices for trustless gameplay.

## Overview

This demo implements fair two-player games where:

- Players commit to moves without revealing them
- An oracle generates adaptor signatures for all possible outcomes
- The winner can claim their prize by completing the adaptor signature
- No party can cheat - the cryptography enforces fair play

## Supported Games

| Game | Description |
|------|-------------|
| Rock-Paper-Scissors | Classic RPS with cryptographic commitments |
| Guess Number | Player A picks 0-9, Player B guesses |

## Architecture

```
fiber-game/
└── crates/
    ├── fiber-game-core/       # Core protocol and game logic
    │   ├── crypto/            # Commitments, signatures (re-exports fiber-core)
    │   ├── fiber/             # FiberClient trait (re-exports fiber-core)
    │   ├── games/             # Game definitions (RPS, Guess Number)
    │   └── protocol/          # Game protocol state machine
    ├── fiber-game-oracle/     # Oracle HTTP service
    └── fiber-game-player/     # Player HTTP service
```

## How It Works

### Game Flow

```
Player A                  Oracle                  Player B
    │                        │                        │
    │  1. Create game        │                        │
    │  (stake + commitment)  │                        │
    │───────────────────────>│                        │
    │                        │                        │
    │                        │  2. Join game          │
    │                        │  (stake + commitment)  │
    │                        │<───────────────────────│
    │                        │                        │
    │  3. Generate adaptor signatures for all outcomes│
    │<───────────────────────│───────────────────────>│
    │                        │                        │
    │  4. Reveal moves       │  4. Reveal moves       │
    │───────────────────────>│<───────────────────────│
    │                        │                        │
    │  5. Oracle determines winner                    │
    │  6. Winner completes adaptor signature          │
    │  7. Winner claims prize via hold invoice        │
```

### Key Cryptographic Components

- **Commitment**: `SHA256(move || salt)` - hides the move until reveal
- **Adaptor Signature**: Partial signature that becomes valid when a secret is revealed
- **Hold Invoice**: Payment locked until preimage (derived from adaptor) is revealed

## Running the Demo

### Start Services

```bash
# Terminal 1: Oracle service (http://localhost:3001)
cargo run -p fiber-game-oracle

# Terminal 2: Player service (http://localhost:3002)
cargo run -p fiber-game-player
```

### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_full_rps_game_a_wins

# With output
cargo test -- --nocapture
```

## API Endpoints

### Oracle (`localhost:3001`)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/games` | Create a new game |
| POST | `/games/{id}/join` | Join an existing game |
| POST | `/games/{id}/reveal` | Reveal your move |
| GET | `/games/{id}` | Get game state |

### Player (`localhost:3002`)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/play` | Play a game (handles full flow) |
| GET | `/balance` | Get current balance |

## Dependencies

This crate depends on `fiber-core` for:
- `Preimage` / `PaymentHash` types
- `FiberClient` trait and `MockFiberClient`

## Testing

The test suite covers:
- Cryptographic primitives (commitments, hashing)
- Game logic (win/lose/draw conditions)
- Full game flows (both players complete a game)
- Edge cases (invalid moves, timeouts)

```bash
# Run all 38 tests
cargo test

# Run only core library tests
cargo test -p fiber-game-core
```

## License

MIT
