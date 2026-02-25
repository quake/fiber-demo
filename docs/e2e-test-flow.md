# End-to-End Test Flow

This document describes how to manually test the fiber-game system with two players completing a full game.

## Prerequisites

- Rust toolchain installed
- One terminal window (combined demo) or three terminal windows (standalone)

## Architecture

### Combined Demo (Recommended)

```
┌─────────────────────────────────────────────────────────────┐
│                Combined Demo Service (:3000)                │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Oracle   │  │  Player A    │  │  Player B    │          │
│  │  /api/    │  │  /api/       │  │  /api/       │          │
│  │  oracle/* │  │  player-a/*  │  │  player-b/*  │          │
│  └──────────┘  └──────────────┘  └──────────────┘          │
│                    (no Fiber RPC calls)                      │
└─────────────────────────────────────────────────────────────┘
         ▲                                      ▲
         │  HTTP                         HTTP   │
         ▼                                      ▼
┌──────────────────┐                 ┌──────────────────┐
│  Player A Browser│                 │  Player B Browser│
│  Fiber RPC ──────┼──►Fiber Node A  │  Fiber RPC ──────┼──►Fiber Node B
└──────────────────┘                 └──────────────────┘
```

### Standalone (3 services)

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Player A      │     │     Oracle      │     │   Player B      │
│   :3001         │────>│     :3000       │<────│   :3002         │
│   Web UI        │     │   Game State    │     │   Web UI        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

## Step 1: Start Services

### Option A: Combined Demo (One Terminal)

```bash
cd fiber-game/crates/fiber-game-demo && cargo run
```

Expected output:
```
Combined demo server starting on http://0.0.0.0:3000
```

Open two browser windows to http://localhost:3000, select "Player A" in one and "Player B" in the other.

#### With Real Fiber Nodes

```bash
FIBER_PLAYER_A_RPC_URL=http://localhost:8227 \
FIBER_PLAYER_B_RPC_URL=http://localhost:8229 \
cargo run
```

The RPC URLs are passed to the frontend — the backend makes no Fiber calls.

### Option B: Standalone (Three Terminals)

Both standalone services use the same frontend-driven architecture as the combined demo — backends manage state only, frontends call Fiber nodes directly.

#### Terminal 1 - Oracle (port 3000)

```bash
cd fiber-game/crates/fiber-game-oracle && cargo run
```

#### Terminal 2 - Player A (port 3001)

```bash
cd fiber-game/crates/fiber-game-player && PORT=3001 cargo run
```

#### Terminal 3 - Player B (port 3002)

```bash
cd fiber-game/crates/fiber-game-player && PORT=3002 cargo run
```

## Step 2: Open Web UIs

- **Combined demo**: http://localhost:3000 (use player selector dropdown)
- **Standalone**: Player A: http://localhost:3001, Player B: http://localhost:3002

Both should show the game lobby with:
- "Create New Game" button
- "Available Games" section (empty initially)
- "My Games" section (empty initially)

## Step 3: Create a Game (Player A)

1. In Player A's browser:
   - Click "Create New Game"
   - Select game type: "Rock Paper Scissors"
   - Enter amount: 1000 shannons
   - Click "Create Game"

2. Expected result:
   - Game appears in "My Games" with status "Waiting for opponent"
   - Game also appears in "Available Games" (visible to both players)

## Step 4: Join the Game (Player B)

1. In Player B's browser:
   - Refresh to see available games
   - Click "Join" on the game created by Player A

2. Expected result:
   - Game moves to "My Games" for Player B
   - Status shows "In Progress" or "Your turn"

## Step 5: Fiber Invoice Setup (Real Fiber Mode Only)

When Fiber RPC URLs are configured, the frontend handles the invoice exchange automatically:

1. **Each player's browser** creates a hold invoice on their own Fiber node using the opponent's `payment_hash` (via `new_invoice` JSON-RPC)
2. **Each player's browser** submits the invoice string to the Oracle backend
3. **Each player's browser** retrieves the opponent's invoice and pays it on their own Fiber node (via `send_payment` JSON-RPC)
4. **Each player's browser** notifies the backend that payment is done

In mock mode (no Fiber URLs), this step is skipped entirely.

## Step 6: Play Moves

### Player A makes a move:
1. Click on the game to open it
2. Select "Rock" (or any move)
3. Click "Play"
4. Status updates to "Waiting for opponent..."

### Player B makes a move:
1. Click on the game to open it
2. Select "Scissors" (or any move)
3. Click "Play"
4. Both players should see the result within 2-4 seconds (polling)

## Step 7: View Results

After both players reveal, the game modal should show:

**For Player A (winner in Rock vs Scissors):**
```
Result: You Won!
Your move: Rock
Opponent's move: Scissors
[Settle] button available
```

**For Player B (loser):**
```
Result: You Lost
Your move: Scissors
Opponent's move: Rock
[Settle] button available
```

## Step 8: Settlement

### Mock Mode
1. **Winner (Player A)** clicks "Settle": Balance increases by 1000 shannons
2. **Loser (Player B)** clicks "Settle": Balance decreases by 1000 shannons

### Real Fiber Mode
1. **Winner (Player A)** clicks "Settle": Browser calls `settle_invoice` on Player A's Fiber node using opponent's preimage (revealed by Oracle). Hold invoice is settled, winner receives payment.
2. **Loser (Player B)** clicks "Settle": Browser calls `cancel_invoice` on Player B's Fiber node. Hold invoice is cancelled, locked funds are returned.

## Verification Points

| Step | Expected Behavior |
|------|-------------------|
| Service starts | Logs port, listens for connections |
| Create game | Returns game_id, game visible in lobby |
| Join game | Both players see game as "In Progress" |
| Invoice exchange (Fiber mode) | Each browser creates invoice on own node, pays opponent's invoice |
| Play move | Commit + reveal sent to Oracle atomically |
| Both revealed | Oracle computes result, returns to both |
| Settlement | Winner settles invoice, loser cancels (Fiber mode); balances adjusted (mock mode) |

## API Verification (curl)

### Check Oracle health (combined demo)
```bash
curl http://localhost:3000/api/oracle/pubkey
```

### List available games
```bash
curl http://localhost:3000/api/oracle/games/available
```

### Check game status
```bash
curl http://localhost:3000/api/oracle/game/{game_id}/status
```

### Check player's games (combined demo)
```bash
curl http://localhost:3000/api/player-a/games/mine
curl http://localhost:3000/api/player-b/games/mine
```

## Troubleshooting

### Game stuck in "Waiting for opponent"
- Ensure Player B joined the game
- Check Oracle logs for errors

### Move not registering
- Check service logs for HTTP errors
- Verify Oracle is running and accessible

### Result not appearing
- Polling occurs every 2 seconds
- Check browser console for JavaScript errors
- Verify both players have revealed (check Oracle logs)

### Fiber invoice/payment errors
- Check browser console for JSON-RPC errors
- Verify Fiber nodes are running and accessible
- Check CORS configuration if browser cannot reach Fiber nodes
- Confirm channel has sufficient balance

### Settlement fails
- Ensure the game has a result (both players revealed)
- In Fiber mode, check that the preimage was revealed by Oracle
- Check browser console for settle_invoice/cancel_invoice RPC errors

## Game Types

### Rock-Paper-Scissors
- Actions: Rock, Paper, Scissors
- Rules: Rock > Scissors > Paper > Rock
- No Oracle secret required

### Guess the Number
- Actions: Number 0-99
- Rules: Oracle picks a secret number, both players guess, player closest to Oracle's number wins
- Oracle commits to secret number at game creation

## Sequence Diagram

```
Player A Browser      Backend (Oracle)      Player B Browser
    │                      │                      │
    │──create game────────►│                      │
    │◄──game_id────────────│                      │
    │                      │◄────join game─────────│
    │                      │────confirmed─────────►│
    │                      │                      │
    │  [Fiber mode: create invoices on own nodes,  │
    │   exchange via Oracle, pay opponent's invoice]│
    │                      │                      │
    │──play(Rock)─────────►│                      │
    │  (commit+reveal)     │                      │
    │                      │◄───play(Scissors)─────│
    │                      │    (commit+reveal)    │
    │                      │                      │
    │        [Oracle judges: A wins]               │
    │                      │                      │
    │──get result─────────►│◄────get result────────│
    │◄──A wins + preimage──│────A wins────────────►│
    │                      │                      │
    │  settle_invoice      │     cancel_invoice    │
    │  (on own Fiber node) │     (on own Fiber node)
    │                      │                      │
```
