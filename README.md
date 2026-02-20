# Fiber Demo

Demo applications showcasing [Fiber Network](https://fiber.nervos.org/) payment channel capabilities on CKB.

## Projects

| Project | Description |
|---------|-------------|
| [fiber-core](./fiber-core/) | Shared library with crypto primitives and FiberClient trait |
| [fiber-game](./fiber-game/) | Two-player game protocol (Rock-Paper-Scissors, Guess Number) |
| [fiber-escrow](./fiber-escrow/) | Escrow trading system with hold invoice-based payment |

## Quick Start

### Prerequisites

- Rust 1.75+ ([rustup](https://rustup.rs/))
- `curl`, `tar`, `jq` (for the setup script)

### 1. Setup Fiber Testnet Nodes

The setup script automatically downloads Fiber binaries, creates accounts, and starts two connected nodes:

```bash
./scripts/setup-fiber-testnet.sh
```

**What the script does:**
1. Downloads `fnn` (Fiber Node) v0.7.0 and `ckb-cli` v2.0.0
2. Creates two CKB accounts
3. Displays addresses for funding via [CKB Faucet](https://faucet.nervos.org)
4. Waits for funding (auto-checks every 10 seconds)
5. Starts two local Fiber nodes (NodeA: port 8227, NodeB: port 8229)
6. Opens a 500 CKB channel between the nodes

**Other commands:**
```bash
./scripts/setup-fiber-testnet.sh status  # Check node and channel status
./scripts/setup-fiber-testnet.sh stop    # Stop running nodes
```

### 2. Run Demo Applications

Once nodes are running, start either demo:

**Escrow Demo** (http://localhost:3000):
```bash
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 \
FIBER_BUYER_RPC_URL=http://localhost:8229 \
cargo run
```

**Game Demo** (http://localhost:3000):
```bash
cd fiber-game/crates/fiber-game-demo
FIBER_PLAYER_A_RPC_URL=http://localhost:8227 \
FIBER_PLAYER_B_RPC_URL=http://localhost:8229 \
cargo run
```

### Mock Mode (No Fiber Nodes)

Both demos can run without real Fiber nodes for testing:

```bash
# Escrow
cd fiber-escrow/crates/fiber-escrow-service && cargo run

# Game
cd fiber-game/crates/fiber-game-demo && cargo run
```

## Documentation

See each project's README for detailed usage:
- [fiber-game/README.md](./fiber-game/README.md) - Game protocol, hold invoice model, API
- [fiber-escrow/README.md](./fiber-escrow/README.md) - Escrow flow, dispute resolution, API

## License

MIT
