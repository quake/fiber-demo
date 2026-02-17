# Decentralized Two-Player Game Protocol Design

## Overview

A decentralized gaming protocol based on Fiber Network Hold Invoices, Oracle signatures, and Adaptor Signatures. Two parties play a game, and the winner automatically obtains the loser's preimage to settle the hold invoice and claim the funds.

**Key Properties:**
- Minimal trust in Oracle (only signs results, cannot steal funds)
- No collateral/deposit required for players
- Verifiable Oracle signatures (fraud is publicly provable)
- Timeout results in draw (no party can exploit by knowing result early)
- Extensible to various game types

## Technical Stack

- **Hold Invoice**: Lock funds until game concludes
- **Adaptor Signature**: Bind preimage release to Oracle's signature
- **Oracle**: Signs game results without holding funds or knowing stakes
- **Commit-Reveal**: Ensure fair play with simultaneous reveal via Oracle

## Participants

- **A**: Player A
- **B**: Player B
- **Oracle**: Trusted third party with minimal responsibilities

## Prerequisites

- Both parties have Fiber Network nodes
- Oracle has published its public key and commitment point
- Both parties agree on which Oracle to use

---

## Generic Protocol Phases

### Phase 1: Initialization

```
Oracle publishes:
  O: Oracle's public key
  R: Commitment point for this game session
  game_id: Unique identifier for this game
  game_type: Type of game (e.g., "rock-paper-scissors", "guess-number")
  oracle_commitment: (optional) Commitment to Oracle's secret input, if game requires

A generates:
  preimage_A ← random(256 bits)
  payment_hash_A = H(preimage_A)
  action_A: Player A's game action (game-specific)
  salt_A ← random(256 bits)

B generates:
  preimage_B ← random(256 bits)
  payment_hash_B = H(preimage_B)
  action_B: Player B's game action (game-specific)
  salt_B ← random(256 bits)
```

### Phase 2: Exchange Hold Invoices

```
A → B: hold_invoice_A (amount X, hash = payment_hash_A)
B → A: hold_invoice_B (amount X, hash = payment_hash_B)

Both parties pay each other's hold invoice
Funds are now locked on both sides
```

### Phase 3: Create Adaptor Signatures and Encrypted Preimages

```
Compute Oracle's future signature points:
  sig_point_A_wins = R + H(R || O || game_id || "A wins") * O
  sig_point_B_wins = R + H(R || O || game_id || "B wins") * O
  sig_point_Draw = R + H(R || O || game_id || "Draw") * O

A creates (for when A loses):
  encrypted_preimage_A = preimage_A XOR H(sig_point_B_wins)

B creates (for when B loses):
  encrypted_preimage_B = preimage_B XOR H(sig_point_A_wins)

Exchange:
  A → B: encrypted_preimage_A
  B → A: encrypted_preimage_B
```

### Phase 4: Commit

```
A → B: commit_A = H(action_A || salt_A)
B → A: commit_B = H(action_B || salt_B)

Both parties confirm receipt of opponent's commit
```

### Phase 5: Simultaneous Reveal (via Oracle)

```
A → Oracle: reveal_A = (action_A, salt_A, commit_A, commit_B, game_id)
B → Oracle: reveal_B = (action_B, salt_B, commit_A, commit_B, game_id)

Oracle waits until:
  - Both reveals received, OR
  - Timeout reached
```

### Phase 6: Oracle Determines Result and Signs

```
Case 1: Both parties revealed

  Oracle verifies:
    H(action_A || salt_A) == commit_A
    H(action_B || salt_B) == commit_B

  Oracle determines result using game-specific rules:
    result = judge(game_type, action_A, action_B, oracle_secret?)
    
    where result ∈ {"A wins", "B wins", "Draw"}

  Oracle signs complete message:
    message = game_id || game_data || result
    sig = Sign(oracle_private_key, message)
    
    (game_data includes all information needed to verify the result)

  Oracle publishes:
    Oracle → All: (game_id, game_data, result, sig)

Case 2: Timeout (one or both parties did not reveal)

  Oracle signs:
    message = game_id || "timeout" || "Draw"
    sig = Sign(oracle_private_key, message)

  Oracle publishes:
    Oracle → All: (game_id, "timeout", "Draw", sig)
```

### Phase 7: Settlement

```
A wins:
  A receives Oracle's signature
  A extracts signature secret from sig
  A decrypts: preimage_B = encrypted_preimage_B XOR H(sig_point_A_wins)
  A verifies: H(preimage_B) == payment_hash_B
  A settles B's hold invoice using preimage_B

B wins:
  (Symmetric operation)

Draw:
  Both parties cancel their own hold invoices
  Funds are refunded to original owners
```

---

## Fraud Proof Mechanism

```
Anyone can verify Oracle honesty:

Input: (game_id, game_data, result, sig)

Verification:
  1. Verify sig is valid signature by Oracle
  2. Verify result == judge(game_type, game_data)

If step 1 passes but step 2 fails:
  → Public cryptographic proof of Oracle fraud
  → Oracle's reputation destroyed
  → If Oracle staked deposit, deposit is slashed
```

---

## Example 1: Rock-Paper-Scissors

### Game Definition

```
game_type: "rock-paper-scissors"
action_space: {Rock, Scissors, Paper}
oracle_secret: None (not needed)

judge(action_A, action_B):
  if (action_A, action_B) ∈ {(Rock, Scissors), (Scissors, Paper), (Paper, Rock)}:
    return "A wins"
  if (action_A, action_B) ∈ {(Scissors, Rock), (Paper, Scissors), (Rock, Paper)}:
    return "B wins"
  if action_A == action_B:
    return "Draw"
```

### Phase 1: Initialization

```
Oracle publishes:
  O: Oracle's public key
  R: Commitment point
  game_id: "rps_12345"
  game_type: "rock-paper-scissors"

A generates:
  preimage_A ← random(256 bits)
  payment_hash_A = H(preimage_A)
  action_A = "Rock"
  salt_A ← random(256 bits)

B generates:
  preimage_B ← random(256 bits)
  payment_hash_B = H(preimage_B)
  action_B = "Scissors"
  salt_B ← random(256 bits)
```

### Phases 2-5

(Same as generic protocol)

### Phase 6: Oracle Signature

```
Oracle receives:
  action_A = "Rock"
  action_B = "Scissors"

Oracle verifies commits match

Oracle judges:
  result = judge("Rock", "Scissors") = "A wins"

Oracle signs:
  game_data = {
    action_A: "Rock",
    action_B: "Scissors"
  }
  message = "rps_12345" || "Rock" || "Scissors" || "A wins"
  sig = Sign(message)

Oracle publishes:
  (game_id, action_A, action_B, result, sig)
```

### Fraud Verification

```
Anyone can verify:
  1. sig is valid Oracle signature on message
  2. judge("Rock", "Scissors") == "A wins" ✓

If Oracle had signed "B wins":
  judge("Rock", "Scissors") == "A wins" ≠ "B wins"
  → Fraud proven
```

---

## Example 2: Guess the Number

### Game Definition

```
game_type: "guess-number"
action_space: {0, 1, 2, ..., 99}
oracle_secret: secret_number ∈ {0, 1, 2, ..., 99}

judge(action_A, action_B, secret_number):
  distance_A = |action_A - secret_number|
  distance_B = |action_B - secret_number|
  
  if distance_A < distance_B:
    return "A wins"
  if distance_B < distance_A:
    return "B wins"
  if distance_A == distance_B:
    return "Draw"
```

### Phase 1: Initialization

```
Oracle generates secret:
  secret_number ← random(0..99)
  oracle_nonce ← random(256 bits)
  oracle_commitment = H(secret_number || oracle_nonce)

Oracle publishes:
  O: Oracle's public key
  R: Commitment point
  game_id: "guess_67890"
  game_type: "guess-number"
  oracle_commitment: H(secret_number || oracle_nonce)  // Hides secret_number

A generates:
  preimage_A ← random(256 bits)
  payment_hash_A = H(preimage_A)
  action_A = 42  // A's guess
  salt_A ← random(256 bits)

B generates:
  preimage_B ← random(256 bits)
  payment_hash_B = H(preimage_B)
  action_B = 55  // B's guess
  salt_B ← random(256 bits)
```

### Phases 2-5

(Same as generic protocol)

### Phase 6: Oracle Signature

```
Oracle receives:
  action_A = 42
  action_B = 55

Oracle verifies commits match

Oracle reveals and judges:
  secret_number = 50
  oracle_nonce = (the nonce used in commitment)
  
  // Anyone can verify: H(50 || oracle_nonce) == oracle_commitment
  
  distance_A = |42 - 50| = 8
  distance_B = |55 - 50| = 5
  
  result = "B wins" (B is closer)

Oracle signs:
  game_data = {
    secret_number: 50,
    oracle_nonce: ...,
    action_A: 42,
    action_B: 55
  }
  message = "guess_67890" || 50 || oracle_nonce || 42 || 55 || "B wins"
  sig = Sign(message)

Oracle publishes:
  (game_id, secret_number, oracle_nonce, action_A, action_B, result, sig)
```

### Fraud Verification

```
Anyone can verify:

1. Oracle commitment was honest:
   H(secret_number || oracle_nonce) == oracle_commitment ✓

2. Result is correct:
   |42 - 50| = 8
   |55 - 50| = 5
   5 < 8, so "B wins" ✓

3. Signature is valid ✓

If any check fails → Fraud proven
```

### Security: Why Oracle Cannot Cheat

```
Oracle's possible cheating:
  After seeing action_A=42, action_B=55, pick secret_number to favor one player

Prevention:
  oracle_commitment = H(secret_number || oracle_nonce) is published BEFORE players commit
  
  Oracle MUST reveal (secret_number, oracle_nonce) that matches the commitment
  Oracle cannot change secret_number after seeing player actions
```

---

## Extending to Other Game Types

### Game Type Requirements

To add a new game type, define:

```
1. action_space: Valid actions for players
2. oracle_secret: (optional) Oracle's hidden input
3. judge(action_A, action_B, oracle_secret?): Deterministic function returning result
4. game_data: All information needed to verify the result
```

### Example: Coin Flip (Oracle decides)

```
game_type: "coin-flip"
action_space: {Heads, Tails}
oracle_secret: coin_result ∈ {Heads, Tails}

judge(action_A, action_B, coin_result):
  // Only A's guess matters, B is just participating for the bet
  if action_A == coin_result:
    return "A wins"
  else:
    return "B wins"
```

### Example: High Card

```
game_type: "high-card"
action_space: {1, 2, 3, ..., 13}  // Card values
oracle_secret: None

judge(action_A, action_B):
  if action_A > action_B:
    return "A wins"
  if action_B > action_A:
    return "B wins"
  return "Draw"
```

### Example: Dice Sum (Over/Under)

```
game_type: "dice-over-under"
action_space: {"Over", "Under"}  // Over 7 or Under 7
oracle_secret: (die1, die2) ∈ {1..6} x {1..6}

judge(action_A, action_B, die1, die2):
  sum = die1 + die2
  actual = "Over" if sum > 7 else ("Under" if sum < 7 else "Draw")
  
  A_correct = (action_A == actual)
  B_correct = (action_B == actual)
  
  if A_correct and not B_correct:
    return "A wins"
  if B_correct and not A_correct:
    return "B wins"
  return "Draw"  // Both correct, both wrong, or sum == 7
```

---

## Oracle Security Analysis

### Minimal Trust Model

```
Oracle's role:
  1. (Optional) Generate and commit to secret before game
  2. Receive player reveals
  3. Verify commits
  4. Apply deterministic judge function
  5. Sign result

Oracle CANNOT:
  - Steal funds (doesn't know preimages)
  - Forge player actions (commits are signed by players)
  - Change secret after seeing actions (commitment published first)
  
Oracle CAN (malicious):
  - Sign wrong result
  - But this is PUBLICLY VERIFIABLE as fraud
```

### Multi-Oracle Extension

```
For higher security, use k-of-n Oracles:

Setup:
  n independent Oracles each publish commitment
  
Signing:
  Each Oracle signs independently
  Result is valid if k Oracles sign the same result
  
Security:
  Attacker must corrupt k Oracles to cheat
```

---

## Message Formats

### Generic Reveal Message

```json
{
  "phase": 5,
  "game_id": "string",
  "game_type": "string",
  "player": "A" | "B",
  "action": "game-specific value",
  "salt": "bytes32",
  "commit_A": "bytes32",
  "commit_B": "bytes32"
}
```

### Generic Oracle Signature Message

```json
{
  "phase": 6,
  "game_id": "string",
  "game_type": "string",
  "game_data": {
    "action_A": "...",
    "action_B": "...",
    "oracle_secret": "... (if applicable)",
    "oracle_nonce": "... (if applicable)"
  },
  "result": "A wins" | "B wins" | "Draw",
  "signature": "bytes64"
}
```

---

## Implementation Considerations

### Oracle Implementation

```
Oracle service requirements:
  1. Secure key management
  2. Secure random number generation (for games requiring oracle_secret)
  3. High availability
  4. Deterministic result calculation
  5. Public API for game creation and reveal submission
  6. Public log of all commitments and signed results
```

### Timeout Parameters

| Parameter | Recommended Value | Rationale |
|-----------|-------------------|-----------|
| Commit timeout | 2 minutes | Time for both players to commit |
| Reveal timeout | 5 minutes | Time for both players to reveal |
| Hold invoice expiry | 1 hour | Time to complete game + settlement |
| Oracle response time | < 30 seconds | User experience |

### Error Handling

| Error | Handling |
|-------|----------|
| Invalid action | Oracle rejects, timeout = Draw |
| Commit mismatch | Oracle rejects reveal, timeout = Draw |
| Oracle commitment mismatch | Fraud, game cancelled |
| Network timeout | Draw (safe default) |

---

## Future Work

1. **Decentralized Oracle Network**: Multiple independent Oracles with k-of-n signing
2. **Oracle Staking**: Economic security through deposits
3. **Multi-round Games**: Support for games with multiple turns
4. **Complex Game State**: Chess, Poker, etc. with state transitions
5. **Privacy Enhancements**: Hide game type and stakes from Oracle
6. **Fiber Network Port**: Adapt for CKB-based Fiber Network
7. **Mobile Implementation**: Optimized for mobile Lightning wallets
8. **Tournament Mode**: Multiple players, bracket-style competitions
