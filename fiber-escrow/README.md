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

**Key Design Points:**

1. **Escrow-holds-preimage model**: Buyer submits preimage when creating order. Escrow stores it and reveals it via API when the order is completed (buyer confirms, dispute resolved to seller, or timeout).

2. **Frontend Fiber calls**: The seller's browser creates hold invoices on the seller's Fiber node. The buyer's browser sends payment on the buyer's Fiber node. The seller's browser settles invoices using the preimage revealed by the escrow.

3. **`/api/config` endpoint**: Returns Fiber RPC URLs to the frontend so browsers know which nodes to call.

4. **Pre-configured products**: The marketplace has 3 hardcoded demo products that remain available after purchase.

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
    │                         │   (seller's browser     │
    │                         │    creates hold invoice  │
    │                         │    on seller's node via  │
    │                         │    new_invoice RPC,      │
    │                         │    then POSTs invoice    │
    │                         │    string to escrow)     │
    │                         │                         │
    │  3. Pay invoice         │                         │
    │  (send_payment RPC      │                         │
    │   on buyer's own node)  │                         │
    │                         │                         │
    │  4. Notify escrow ─────►│                         │
    │  (POST /pay)            │                         │
    │◄── status: funded ──────│                         │
    │                         │                         │
    │  [Seller ships]         │                         │
    │                         │                         │
    │  5. Confirm receipt ───►│                         │
    │                         │── preimage revealed ───►│
    │                         │   in order details      │
    │                         │                         │
    │                         │  6. Seller settles      │
    │                         │  (settle_invoice RPC    │
    │                         │   on seller's own node) │
    │                         │◄── status: completed ───│
```

### Dispute Flow

If the buyer disputes, the arbiter reviews and decides:
- **To Seller**: Escrow reveals preimage in order details. Seller's browser calls `settle_invoice` on seller's node.
- **To Buyer**: Escrow updates order to refunded. Seller's browser calls `cancel_invoice` on seller's node. Buyer's funds are refunded.

### Timeout Protection

If the buyer doesn't confirm within the timeout period (default 7 days), the escrow automatically marks the order as completed and reveals the preimage. The seller can then settle the invoice on their Fiber node.

## Running the Demo

### Mock Mode (No Fiber Nodes)

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Service at http://localhost:3000
# Fiber operations are skipped, backend manages state independently
```

### Real Fiber Mode

```bash
# Terminal 1: Start Fiber testnet nodes
./scripts/setup-fiber-testnet.sh

# Terminal 2: Start escrow (URLs are passed to frontend, not used by backend)
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 \
FIBER_BUYER_RPC_URL=http://localhost:8229 \
cargo run
```

Then open http://localhost:3000 to use the demo.

## Web UI

```
┌─────────────────────────────────────────────────────────────────┐
│  Fiber Escrow Demo                [User: alice ▼] [Balance]     │
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
| Digital Art NFT | 1000 shannons | bob |
| E-book:Erta Programming | 500 shannons | bob |
| Music Album | 800 shannons | carol |

Products remain available after purchase (can be bought multiple times).

### Tabs

- **Market**: Browse and buy available products
- **My Orders**: View orders as buyer/seller, create invoices (seller), pay invoices (buyer), confirm receipt, dispute, mark shipped
- **Arbiter**: Resolve disputes, simulate time passage

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `3000` |
| `FIBER_SELLER_RPC_URL` | Seller's Fiber node RPC URL (passed to frontend) | (none - mock mode) |
| `FIBER_BUYER_RPC_URL` | Buyer's Fiber node RPC URL (passed to frontend) | (none - mock mode) |

When Fiber RPC URLs are set, the backend passes them to the frontend via `/api/config`. The frontend then makes Fiber RPC calls directly from the browser. The backend itself **never** connects to Fiber nodes.

## API Endpoints

### Configuration

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/config` | Get Fiber RPC URLs for frontend |
| GET | `/api/health` | Health check |

### Users

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/user/register` | Register a new user |
| GET | `/api/user/me` | Get current user info |
| GET | `/api/users` | List all users |

### Products

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/products` | Create a product |
| GET | `/api/products` | List all products |
| GET | `/api/products/mine` | List my products |

### Orders

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/orders` | Create order with preimage |
| GET | `/api/orders/mine` | List my orders |
| GET | `/api/orders/{id}` | Get order details (includes preimage when completed) |
| POST | `/api/orders/{id}/invoice` | Submit invoice string (seller) |
| POST | `/api/orders/{id}/pay` | Notify payment completed (buyer) |
| POST | `/api/orders/{id}/ship` | Mark as shipped (seller) |
| POST | `/api/orders/{id}/confirm` | Confirm receipt (buyer, reveals preimage) |
| POST | `/api/orders/{id}/dispute` | Open dispute (buyer) |

### Create Order Request

```json
{
  "product_id": "uuid",
  "preimage": "0x<64 hex chars>"
}
```

The escrow computes `payment_hash = Blake2b-256(preimage)` and stores the preimage. The `payment_hash` is returned in the response for the seller to create a hold invoice.

### Submit Invoice Request

```json
{
  "invoice": "fibt1..."
}
```

The seller's browser creates a hold invoice on the seller's Fiber node using the `payment_hash`, then submits the invoice string to the escrow via this endpoint.

### Arbiter

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/arbiter/disputes` | List open disputes |
| POST | `/api/arbiter/disputes/{id}/resolve` | Resolve: `{"resolution": "seller"}` or `{"resolution": "buyer"}` |

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

## Dependencies

- `fiber-core`: `Preimage`, `PaymentHash` types (crypto primitives only)
- `axum`: HTTP server
- `tower-http`: CORS and static file serving

Note: `reqwest` is **not** a runtime dependency — it is only used in dev-dependencies for e2e tests. The backend makes no outbound HTTP calls at runtime.

## Order Status Flow

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
                   [seller settles                      │             [seller settles
                    on own node]            ┌───────────┴──────────┐   on own node]
                                            ▼                      ▼
                                       Completed               Refunded
                                     (arbiter: seller)      (arbiter: buyer)
                                     [preimage revealed]    [seller cancels
                                     [seller settles         on own node]
                                      on own node]
```

## CORS Note

Since the frontend calls Fiber nodes directly from the browser, CORS restrictions may apply. See [CORS considerations](../docs/escrow-e2e-test-flow.md#cors-considerations) for solutions.

## License

MIT
