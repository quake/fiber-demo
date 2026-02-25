# Fiber Escrow

An escrow trading system built on Fiber Network using hold invoices for secure buyer-seller transactions.

## Overview

This demo implements a marketplace where:

- **Sellers** list products for sale (pre-configured demo products)
- **Buyers** purchase with funds locked in hold invoices
- **Escrow** holds the preimage and reveals it to settle payment
- **Arbiter** resolves disputes
- **Automatic timeout** protects sellers from unresponsive buyers

## Architecture

The backend makes **zero** Fiber RPC calls. All Fiber Network interactions happen in the browser:

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   Buyer's    │         │    Escrow    │         │   Seller's   │
│   Browser    │         │   Backend    │         │   Browser    │
│              │◄───────►│  (pure HTTP  │◄───────►│              │
│  Fiber RPC ──┤         │  state mgmt) │         ├── Fiber RPC  │
└──────┬───────┘         └──────────────┘         └──────┬───────┘
       │                   (no Fiber                     │
       │                    connection)                   │
       ▼                                                 ▼
┌──────────────┐                                  ┌──────────────┐
│   Buyer's    │                                  │   Seller's   │
│  Fiber Node  │                                  │  Fiber Node  │
└──────────────┘                                  └──────────────┘
```

## How It Works

### Normal Flow

```
Buyer Browser            Escrow Backend           Seller Browser
    │                         │                         │
    │  1. Create order        │                         │
    │  (submit preimage) ────►│                         │
    │◄── order + hash ────────│                         │
    │                         │                         │
    │                         │◄── 2. Submit invoice ───│
    │                         │   (seller creates hold  │
    │                         │    invoice on own node   │
    │                         │    via new_invoice RPC)  │
    │                         │                         │
    │  3. Pay invoice         │                         │
    │  (send_payment RPC      │                         │
    │   on buyer's own node)  │                         │
    │                         │                         │
    │  4. Notify escrow ─────►│                         │
    │◄── status: funded ──────│                         │
    │                         │                         │
    │  [Seller ships]         │                         │
    │                         │                         │
    │  5. Confirm receipt ───►│                         │
    │                         │── preimage revealed ───►│
    │                         │                         │
    │                         │  6. Seller settles      │
    │                         │  (settle_invoice RPC    │
    │                         │   on seller's own node) │
```

### Dispute Flow

If the buyer disputes, the arbiter reviews and decides:
- **To Seller**: Escrow reveals preimage. Seller settles invoice on own node.
- **To Buyer**: Seller cancels invoice on own node. Buyer's funds are refunded.

### Timeout Protection

If the buyer doesn't confirm within the timeout period, the escrow automatically completes the order and reveals the preimage. The seller can then settle the invoice.

### Order Status Flow

```
WaitingPayment ──[invoice submitted]──> WaitingPayment ──[pay notified]──> Funded
                                                                              │
                                                                         [ship]
                                                                              │
                                                                              ▼
                                                                          Shipped
                                                                              │
                          ┌───────────────────────────────┬───────────────────┤
                          │                               │                  │
                          ▼                               ▼                  ▼
                     Completed                       Disputed            (timeout)
                   (buyer confirms)              (buyer disputes)     (auto-complete)
                   [preimage revealed]                  │             [preimage revealed]
                   [seller settles                      │
                    on own node]            ┌───────────┴──────────┐
                                            ▼                      ▼
                                       Completed               Refunded
                                     (arbiter: seller)      (arbiter: buyer)
                                     [preimage revealed]    [seller cancels
                                                             on own node]
```

## Running the Demo

### Mock Mode (No Fiber Nodes)

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Service at http://localhost:3000
```

### Real Fiber Mode

```bash
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 \
FIBER_BUYER_RPC_URL=http://localhost:8229 \
cargo run
```

The demo comes with pre-registered users (alice=buyer, bob=seller, carol=arbiter) and demo products. Open http://localhost:3000 to use the Web UI.

### Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `3000` |
| `FIBER_SELLER_RPC_URL` | Seller's Fiber node RPC URL (passed to frontend) | (none - mock mode) |
| `FIBER_BUYER_RPC_URL` | Buyer's Fiber node RPC URL (passed to frontend) | (none - mock mode) |

## Run Tests

```bash
# Run E2E tests
cargo test --test e2e_escrow_flow -- --nocapture
```

## License

MIT
