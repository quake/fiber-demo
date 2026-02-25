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
| Guess Number | Oracle picks a secret number 0-99, both players guess, closest wins |

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

### Frontend-Driven Fiber Integration

The backend makes **zero** Fiber RPC calls. All Fiber Network interactions happen in the browser:

```
┌────────────┐                                    ┌────────────┐
│ Player A   │         ┌──────────────┐           │ Player B   │
│ Browser    │◄───────►│   Backend    │◄─────────►│ Browser    │
│            │  HTTP   │  (Oracle +   │   HTTP    │            │
│ Fiber RPC ─┼─ ─ ─ ─ │  Game State) │ ─ ─ ─ ─ ─┼─ Fiber RPC │
└─────┬──────┘         └──────────────┘           └─────┬──────┘
      │                  (no Fiber)                     │
      ▼                  connection                     ▼
┌────────────┐                                    ┌────────────┐
│ Fiber      │                                    │ Fiber      │
│ Node A     │                                    │ Node B     │
└────────────┘                                    └────────────┘
```

**Frontend responsibilities:**
- `new_invoice` — create hold invoice on own Fiber node
- `send_payment` — pay opponent's invoice on own Fiber node
- `settle_invoice` — settle hold invoice with opponent's preimage (winner)
- `cancel_invoice` — cancel hold invoice (loser, refund)
- `list_channels` — query balance from own Fiber node

**Backend responsibilities:**
- Game state management (create, join, reveal, result)
- Oracle logic (adaptor signatures, winner determination)
- Preimage/payment hash exchange between players
- No Fiber node connection whatsoever

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

### Hold Invoice Flow (Frontend-Driven)

```
Player A Browser          Backend (Oracle)          Player B Browser
    │                          │                          │
    │  Submit preimage_a       │     Submit preimage_b    │
    │  + payment_hash_a ──────►│◄──── + payment_hash_b   │
    │                          │                          │
    │  Get opponent's hash_b   │   Get opponent's hash_a  │
    │◄─────────────────────────│─────────────────────────►│
    │                          │                          │
    │  Create invoice on       │    Create invoice on     │
    │  own Fiber node          │    own Fiber node        │
    │  (using hash_b)          │    (using hash_a)        │
    │  [new_invoice RPC]       │    [new_invoice RPC]     │
    │                          │                          │
    │  Submit invoice string──►│◄── Submit invoice string │
    │                          │                          │
    │  Get opponent's invoice  │  Get opponent's invoice  │
    │◄─────────────────────────│─────────────────────────►│
    │                          │                          │
    │  Pay B's invoice on      │    Pay A's invoice on    │
    │  own Fiber node          │    own Fiber node        │
    │  [send_payment RPC]      │    [send_payment RPC]    │
    │                          │                          │
    │       [Both reveal moves - Oracle determines winner]│
    │                          │                          │
    │  Oracle reveals          │                          │
    │  preimage_b to A ◄───────│                          │
    │                          │                          │
    │  A settles invoice       │                          │
    │  on own Fiber node       │                          │
    │  [settle_invoice RPC]    │                          │
    │                          │                          │
    │                          │    B cancels invoice     │
    │                          │    on own Fiber node     │
    │                          │    [cancel_invoice RPC]  │
    │                          │─────────────────────────►│
```

**Key insight**: Each player's invoice is created on their **own** Fiber node with the **opponent's** `payment_hash`. To settle it, you need the **opponent's preimage**, which the Oracle only reveals to the winner. All Fiber RPC calls are made by the player's browser directly to their own Fiber node.

## Running the Demo

The easiest way to run the demo is using the combined service, which starts the Oracle and two Player UIs on a single port.

### 1. Combined Demo (Recommended)

```bash
# Start combined Oracle + 2 Players (http://localhost:3000)
cd fiber-game/crates/fiber-game-demo && cargo run
```

Open http://localhost:3000 and use the **Player selector** dropdown to switch between Player A and Player B (open two browser windows for two-player testing).

#### Real Fiber Integration
To test with real Fiber nodes, set the RPC URLs. These are passed to the frontend via the backend — the backend itself never connects to Fiber nodes:
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
| `FIBER_PLAYER_A_RPC_URL` | Fiber node RPC URL for Player A (passed to frontend) | None (Mock mode) |
| `FIBER_PLAYER_B_RPC_URL` | Fiber node RPC URL for Player B (passed to frontend) | None (Mock mode) |

## Key Concepts

- **Shannons**: All amounts in this demo use **shannons**, the native unit of CKB (1 CKB = 10^8 shannons).
- **Mock Mode**: By default, the services run in "Mock Mode". Without Fiber RPC URLs, the frontend gracefully skips Fiber operations and the backend manages game state independently.
- **Hold Invoices**: Funds are locked in a Fiber hold invoice when a game starts and only released to the winner upon reveal.

### Hold Invoice Security Model

The game uses hold invoices to lock funds securely:

1. **Payment Hash & Preimage Submission**: Each player generates a random preimage, computes its hash (`payment_hash`), and submits **both** to the Oracle (preimage is kept secret until game ends)
2. **Cross-Invoice Creation**: Players create invoices on their **own** Fiber node using the **opponent's** `payment_hash`, ensuring only the opponent's preimage can settle it
3. **Mutual Payment**: Both players pay each other's invoices from their **own** Fiber node (funds are locked, not transferred)
4. **Oracle Reveals Preimage**: When the game ends, the Oracle reveals the **loser's preimage** to the winner
5. **Winner Settlement**: The winner uses the opponent's preimage to settle their own invoice on their **own** Fiber node (claiming the funds the opponent paid)

#### Oracle Trust Model

**Current Demo (Simplified)**: This demo uses a **trusted Oracle** model for simplicity. The Oracle:
- Stores both players' preimages
- Determines the winner based on revealed moves
- Reveals the loser's preimage to the winner

If the Oracle cheats (e.g., reveals the wrong preimage or lies about the winner), players cannot detect it in this simplified version.

**Production Design (Adaptor Signatures)**: The full protocol uses **adaptor signatures** to make Oracle cheating detectable:

1. Before the game starts, the Oracle commits to a **signature point** for each possible outcome
2. Players verify these commitments match the Oracle's public key
3. When the game ends, the Oracle reveals the **adaptor signature** for the actual outcome
4. Players can verify the signature matches the pre-committed point

If the Oracle tries to cheat:
- **Wrong winner**: The adaptor signature won't match the committed signature point for that outcome
- **Invalid signature**: Players can cryptographically prove the Oracle misbehaved
- **Public accountability**: The Oracle's public key is known, so cheating damages its reputation

The adaptor signature approach is implemented in `fiber-game-core/src/crypto/signature_point.rs` but not yet integrated into the demo's settlement flow.

#### Production Considerations

In this demo, we trust that opponents correctly use the exchanged `payment_hash` from the Oracle. In a production environment, additional verification is needed:

1. **Invoice String Verification**: The opponent should send their actual invoice string (BOLT11/BOLT12 format)
2. **Parse and Validate**: Extract the `payment_hash` from the invoice string and verify it matches your `payment_hash`
3. **Only Then Pay**: Only pay the invoice after verification passes

This prevents a malicious opponent from creating an invoice with a different `payment_hash` that they control.

## Run Tests

```bash
# Run all tests in the workspace
cargo test

# Run specific E2E test (requires building crates first)
cargo test --test e2e_game_flow -- --nocapture
```

## API Endpoints

### Oracle (Combined Demo: `/api/oracle/*`)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/oracle/pubkey` | Get Oracle public key |
| GET | `/api/oracle/games/available` | List available games |
| POST | `/api/oracle/game/create` | Create a new game |
| POST | `/api/oracle/game/{id}/join` | Join an existing game |
| POST | `/api/oracle/game/{id}/payment-hash` | Submit payment hash + preimage |
| GET | `/api/oracle/game/{id}/payment-hash/{player}` | Get opponent's payment hash |
| POST | `/api/oracle/game/{id}/invoice` | Submit invoice string |
| GET | `/api/oracle/game/{id}/invoice/{player}` | Get opponent's invoice string |
| POST | `/api/oracle/game/{id}/encrypted-preimage` | Submit encrypted preimage |
| GET | `/api/oracle/game/{id}/encrypted-preimage/{player}` | Get opponent's encrypted preimage |
| POST | `/api/oracle/game/{id}/commit` | Submit move commitment |
| POST | `/api/oracle/game/{id}/reveal` | Reveal move |
| GET | `/api/oracle/game/{id}/status` | Get game status |
| GET | `/api/oracle/game/{id}/result` | Get game result (includes preimage for winner) |

### Player (Combined Demo: `/api/player-a/*` or `/api/player-b/*`)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/player-{a,b}/player` | Get player info |
| GET | `/api/player-{a,b}/games/available` | List available games |
| GET | `/api/player-{a,b}/games/mine` | List my games |
| POST | `/api/player-{a,b}/game/create` | Create a game |
| POST | `/api/player-{a,b}/game/join` | Join a game |
| POST | `/api/player-{a,b}/game/{id}/play` | Submit move |
| GET | `/api/player-{a,b}/game/{id}/status` | Get game status |
| POST | `/api/player-{a,b}/game/{id}/settle` | Settle after game ends |
| POST | `/api/player-{a,b}/game/{id}/invoice-created` | Notify backend of invoice creation |
| POST | `/api/player-{a,b}/game/{id}/payment-done` | Notify backend of payment completion |

## Dependencies

This crate depends on `fiber-core` for:
- `Preimage` / `PaymentHash` types

Note: The backend does **not** use `FiberClient`, `MockFiberClient`, or `RpcFiberClient` — all Fiber interactions are handled by the frontend.

## Testing

The test suite covers:
- Cryptographic primitives (commitments, hashing)
- Game logic (win/lose/draw conditions)
- Full game flows (both players complete a game)
- Edge cases (invalid moves, timeouts)

```bash
# Run all tests
cargo test

# Run only core library tests
cargo test -p fiber-game-core
```

## License

MIT
