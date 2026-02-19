# Fiber Escrow

An escrow trading system built on Fiber Network using hold invoices for secure buyer-seller transactions.

## Overview

This demo implements a marketplace where:

- **Sellers** list products for sale (pre-configured demo products)
- **Buyers** purchase with funds locked in hold invoices
- **Escrow** holds the preimage and settles/cancels invoices
- **Arbiter** resolves disputes
- **Automatic timeout** protects sellers from unresponsive buyers

## Architecture

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   Buyer's    │         │    Escrow    │         │   Seller's   │
│  Fiber Node  │         │   Service    │         │  Fiber Node  │
└──────┬───────┘         └──────┬───────┘         └──────┬───────┘
       │                        │                        │
       │  FIBER_BUYER_RPC_URL   │  FIBER_SELLER_RPC_URL  │
       │<───────────────────────│                        │
       │                        │                        │
       │  /api/fiber/send_payment                        │
       │  (backend proxy)       │  create_hold_invoice   │
       │                        │  get_invoice           │
       │                        │  settle_invoice        │
       │                        │  cancel_invoice        │
       │                        │  ─────────────────────>│
```

**Key Design Points:**

1. **Escrow-holds-preimage model**: Buyer submits preimage when creating order. Escrow stores it and can settle invoices autonomously (timeout, dispute resolution).

2. **Dual-node connection**: Escrow connects to both seller's node (`FIBER_SELLER_RPC_URL`) and buyer's node (`FIBER_BUYER_RPC_URL`).

3. **Backend payment proxy**: The Web UI calls `/api/fiber/send_payment` which proxies to the buyer's node, avoiding CORS issues.

4. **Pre-configured products**: The marketplace has 3 hardcoded demo products that remain available after purchase.

## How It Works

### Normal Flow

```
Buyer                    Escrow Service              Seller's Node
  │                           │                           │
  │  1. Buy product           │                           │
  │  (submit preimage) ──────>│                           │
  │                           │  2. create_hold_invoice   │
  │                           │  ─────────────────────────>│
  │                           │<──── invoice_string ──────│
  │<── order created ─────────│                           │
  │                           │                           │
  │  3. Confirm & Pay ───────>│                           │
  │                           │  4. send_payment          │
  │                           │  (via buyer's node)       │
  │                           │  ─────────────────────────>│
  │                           │                           │
  │                           │  5. get_invoice           │
  │                           │  ─────────────────────────>│
  │                           │<──── status: Held ────────│
  │<── status: funded ────────│                           │
  │                           │                           │
  │  [Seller ships]           │                           │
  │                           │                           │
  │  6. Confirm receipt ─────>│  7. settle_invoice        │
  │                           │  (using stored preimage)  │
  │                           │  ─────────────────────────>│
  │                           │<──── settled ─────────────│
```

### Dispute Flow

If the buyer disputes, the arbiter reviews and decides:
- **To Seller**: Escrow settles invoice with stored preimage
- **To Buyer**: Escrow cancels invoice, buyer gets refund

### Timeout Protection

If the buyer doesn't confirm within the timeout period (default 7 days), the escrow automatically settles the invoice using the stored preimage.

## Running the Demo

### Mock Mode (No Fiber Nodes)

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Service at http://localhost:3000
# Payments are simulated (trust mode)
```

### Real Fiber Mode

```bash
# Terminal 1: Start Fiber testnet nodes
./scripts/setup-fiber-testnet.sh

# Terminal 2: Start escrow with both nodes configured
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 \
FIBER_BUYER_RPC_URL=http://localhost:8229 \
cargo run
```

Then open http://localhost:3000 to use the demo.

## Web UI

```
┌─────────────────────────────────────────────────────────────────┐
│  Fiber Escrow Demo                [User: alice ▼] [9500 sats]   │
├─────────────────────────────────────────────────────────────────┤
│  [Market]  [My Orders]  [Arbiter]                               │
└─────────────────────────────────────────────────────────────────┘
```

### Demo Users

| User  | Role   |
|-------|--------|
| alice | Buyer  |
| bob   | Seller |
| carol | Arbiter |

### Demo Products

The marketplace comes with 3 pre-configured products:

| Product | Price | Seller |
|---------|-------|--------|
| Digital Art NFT | 1000 sats | bob |
| E-book:Erta Programming | 500 sats | bob |
| Music Album | 800 sats | carol |

Products remain available after purchase (can be bought multiple times).

### Tabs

- **Market**: Browse and buy available products
- **My Orders**: View orders as buyer/seller, pay invoices, confirm receipt, dispute, mark shipped
- **Arbiter**: Resolve disputes, simulate time passage

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `3000` |
| `FIBER_SELLER_RPC_URL` | Seller's Fiber node RPC URL | (none - mock mode) |
| `FIBER_BUYER_RPC_URL` | Buyer's Fiber node RPC URL | (none - mock mode) |

When both `FIBER_SELLER_RPC_URL` and `FIBER_BUYER_RPC_URL` are set, the escrow service connects to real Fiber nodes. Otherwise, it runs in mock mode with simulated payments.

## API Endpoints

### Orders

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/orders` | Create order with preimage |
| GET | `/api/orders/{id}` | Get order details |
| GET | `/api/orders/mine` | List my orders |
| POST | `/api/orders/{id}/pay` | Verify payment (after buyer pays) |
| POST | `/api/orders/{id}/ship` | Mark as shipped |
| POST | `/api/orders/{id}/confirm` | Confirm receipt (settles invoice) |
| POST | `/api/orders/{id}/dispute` | Open dispute |

### Create Order Request

```json
{
  "product_id": "uuid",
  "preimage": "0x<64 hex chars>"
}
```

The escrow computes `payment_hash = SHA256(preimage)` and creates a hold invoice.

### Arbiter

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/arbiter/disputes` | List open disputes |
| POST | `/api/arbiter/disputes/{id}/resolve` | Resolve: `{"resolution": "seller"}` or `{"resolution": "buyer"}` |

### Fiber Proxy

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/fiber/send_payment` | Send payment via buyer's node (backend proxy) |

**Send Payment Request:**
```json
{
  "invoice": "fibt1..."
}
```

This endpoint proxies the payment request to the buyer's Fiber node configured via `FIBER_BUYER_RPC_URL`, avoiding CORS issues.

### System

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/system/tick` | Simulate time: `{"seconds": 86400}` |

## Testing

### Automated E2E Tests

```bash
cd fiber-escrow && cargo test --test e2e_escrow_flow -- --nocapture
```

### Manual Testing

See [E2E Test Flow Documentation](../docs/escrow-e2e-test-flow.md) for detailed test scenarios.

## CORS Note

The escrow service includes a backend proxy (`/api/fiber/send_payment`) that routes payment requests to the buyer's Fiber node. This avoids CORS issues since the browser only communicates with the escrow service, not directly with Fiber nodes.

## Dependencies

- `fiber-core`: `Preimage`, `PaymentHash`, `FiberClient` trait, `RpcFiberClient`
- `axum`: HTTP server
- `tower-http`: CORS and static file serving
- `reqwest`: HTTP client for Fiber RPC calls

## Order Status Flow

```
WaitingPayment ──[pay verified]──> Funded ──[ship]──> Shipped
                                                         │
                          ┌──────────────────────────────┼──────────────────────────────┐
                          │                              │                              │
                          ▼                              ▼                              ▼
                     Completed                      Disputed                       (timeout)
                   (buyer confirms)              (buyer disputes)                (auto-settle)
                   [settle_invoice]                     │                       [settle_invoice]
                                          ┌─────────────┴─────────────┐
                                          ▼                           ▼
                                     Completed                    Refunded
                                   (arbiter: seller)           (arbiter: buyer)
                                   [settle_invoice]            [cancel_invoice]
```

## License

MIT
