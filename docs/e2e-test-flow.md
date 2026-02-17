# End-to-End Test Flow

This document describes how to manually test the fiber-game system with two players completing a full game.

## Prerequisites

- Rust toolchain installed
- Three terminal windows

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Player A      │     │     Oracle      │     │   Player B      │
│   :3001         │────>│     :3000       │<────│   :3002         │
│   Web UI        │     │   Game State    │     │   Web UI        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

## Step 1: Start Services

### Terminal 1 - Oracle (port 3000)

```bash
cd fiber-game
cargo run --bin fiber-game-oracle
```

Expected output:
```
Oracle server starting on http://0.0.0.0:3000
Oracle pubkey: <hex pubkey>
```

### Terminal 2 - Player A (port 3001)

```bash
cd fiber-game
cargo run --bin fiber-game-player -- --port 3001 --oracle-url http://localhost:3000
```

Expected output:
```
Player server starting on http://0.0.0.0:3001
Player ID: <uuid>
Oracle URL: http://localhost:3000
```

### Terminal 3 - Player B (port 3002)

```bash
cd fiber-game
cargo run --bin fiber-game-player -- --port 3002 --oracle-url http://localhost:3000
```

Expected output:
```
Player server starting on http://0.0.0.0:3002
Player ID: <uuid>
Oracle URL: http://localhost:3000
```

## Step 2: Open Web UIs

- Player A: http://localhost:3001
- Player B: http://localhost:3002

Both should show the game lobby with:
- "Create New Game" button
- "Available Games" section (empty initially)
- "My Games" section (empty initially)

## Step 3: Create a Game (Player A)

1. In Player A's browser (http://localhost:3001):
   - Click "Create New Game"
   - Select game type: "Rock Paper Scissors"
   - Enter amount: 1000 sats
   - Click "Create Game"

2. Expected result:
   - Game appears in "My Games" with status "Waiting for opponent"
   - Game also appears in "Available Games" (visible to both players)

## Step 4: Join the Game (Player B)

1. In Player B's browser (http://localhost:3002):
   - Refresh to see available games
   - Click "Join" on the game created by Player A

2. Expected result:
   - Game moves to "My Games" for Player B
   - Status shows "In Progress" or "Your turn"

## Step 5: Play Moves

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

## Step 6: View Results

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

## Step 7: Settlement

1. **Winner (Player A)** clicks "Settle":
   - Balance increases by 1000 sats
   - Status shows "Settled"

2. **Loser (Player B)** clicks "Settle":
   - Balance decreases by 1000 sats
   - Status shows "Settled"

Note: In the current mock implementation, settlement adjusts in-memory balances. With real Fiber integration, this would settle the hold invoices using preimages.

## Verification Points

| Step | Expected Behavior |
|------|-------------------|
| Oracle starts | Logs pubkey, listens on :3000 |
| Players start | Each logs unique player ID |
| Create game | Returns game_id, game visible in lobby |
| Join game | Both players see game as "In Progress" |
| Play move | Commit + reveal sent to Oracle atomically |
| Both revealed | Oracle computes result, returns to both |
| Settlement | Balances adjusted, game marked "Settled" |

## API Verification (curl)

### Check Oracle health
```bash
curl http://localhost:3000/oracle/pubkey
```

### List available games
```bash
curl http://localhost:3000/games/available
```

### Check game status
```bash
curl http://localhost:3000/game/{game_id}/result
```

### Check player's games
```bash
curl http://localhost:3001/api/games/mine
curl http://localhost:3002/api/games/mine
```

## Troubleshooting

### Game stuck in "Waiting for opponent"
- Ensure Player B joined the game
- Check Oracle logs for errors

### Move not registering
- Check player service logs for HTTP errors
- Verify Oracle is running and accessible

### Result not appearing
- Polling occurs every 2 seconds
- Check browser console for JavaScript errors
- Verify both players have revealed (check Oracle logs)

### Settlement fails
- Ensure the game has a result (both players revealed)
- Check player service logs for settle endpoint errors

## Game Types

### Rock-Paper-Scissors
- Actions: Rock, Paper, Scissors
- Rules: Rock > Scissors > Paper > Rock
- No Oracle secret required

### Guess the Number
- Actions: Number 0-99
- Rules: Player closer to Oracle's secret number wins
- Oracle commits to secret number at game creation

## Sequence Diagram

```
Player A          Oracle           Player B
   │                │                  │
   │──create game──>│                  │
   │<──game_id──────│                  │
   │                │<────join game────│
   │                │────confirmed────>│
   │                │                  │
   │──play(Rock)───>│                  │
   │  (commit+reveal)                  │
   │                │<───play(Scissors)│
   │                │    (commit+reveal)
   │                │                  │
   │          [Oracle judges: A wins]  │
   │                │                  │
   │──get status───>│<────get status───│
   │<──A wins───────│─────A wins──────>│
   │                │                  │
   │──settle───────>│                  │
   │<──+1000 sats───│                  │
   │                │<─────settle──────│
   │                │────-1000 sats───>│
```

## Next Steps

After successful E2E testing:
1. Add real Fiber Network integration (replace MockFiberClient)
2. Add timeout handling for unresponsive players
3. Add game history persistence
4. Add more game types
