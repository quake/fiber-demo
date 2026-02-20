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
| Guess Number | Player A picks 0-99, Player B guesses |

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
    ├── fiber-game-player/     # Player HTTP service
    └── fiber-game-demo/       # Combined demo service
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

The easiest way to run the demo is using the combined service, which starts the Oracle and two Player UIs on a single port.

### 1. Combined Demo (Recommended)

```bash
# Start combined Oracle + 2 Players (http://localhost:3000)
cd fiber-game/crates/fiber-game-demo && cargo run
```

Access the player interfaces:
- **Player A**: [http://localhost:3000/player-a/](http://localhost:3000/player-a/)
- **Player B**: [http://localhost:3000/player-b/](http://localhost:3000/player-b/)

#### Real Fiber Integration
To test with real Fiber nodes, set the RPC URLs for each player:
```bash
FIBER_PLAYER_A_RPC_URL=http://localhost:8227 \
FIBER_PLAYER_B_RPC_URL=http://localhost:8229 \
cargo run
```

### 2. Separate Services (Standalone)

If you need to run services independently across different machines or ports:

```bash
# Terminal 1: Oracle service (http://localhost:3000)
cd fiber-game/crates/fiber-game-oracle && cargo run

# Terminal 2: Player A (http://localhost:3001)
cd fiber-game/crates/fiber-game-player && PORT=3001 cargo run

# Terminal 3: Player B (http://localhost:3002)
cd fiber-game/crates/fiber-game-player && PORT=3002 cargo run
```

### Configuration

| Env Variable | Description | Default |
|--------------|-------------|---------|
| `PORT` | HTTP service port | 3000 |
| `ORACLE_URL` | URL of the Oracle service (for players) | http://localhost:3000 |
| `FIBER_RPC_URL` | Fiber node RPC URL (standalone player) | None (Mock mode) |
| `FIBER_PLAYER_A_RPC_URL` | Fiber node RPC for Player A (demo) | None (Mock mode) |
| `FIBER_PLAYER_B_RPC_URL` | Fiber node RPC for Player B (demo) | None (Mock mode) |

## Key Concepts

- **Shannons**: All amounts in this demo use **shannons**, the native unit of CKB (1 CKB = 10^8 shannons).
- **Mock Mode**: By default, the services run in "Mock Mode" with simulated Fiber balances (100,000 shannons initial).
- **Hold Invoices**: Funds are locked in a Fiber hold invoice when a game starts and only released to the winner upon reveal.

## Run Tests

```bash
# Run all tests in the workspace
cargo test

# Run specific E2E test (requires building crates first)
cargo test --test e2e_game_flow -- --nocapture
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
