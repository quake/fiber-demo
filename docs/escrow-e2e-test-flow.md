# Fiber Escrow E2E Test Flow

Manual end-to-end test scenarios for the Fiber Escrow demo.

## Prerequisites

### Mock Mode (No Fiber Nodes)

For quick testing without Fiber nodes:

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Service running at http://localhost:3000
# Fiber operations are skipped, backend manages state independently
```

### Real Fiber Mode (With Fiber Nodes)

For testing with real Fiber Network payments:

```bash
# Terminal 1: Start Fiber testnet nodes
./scripts/setup-fiber-testnet.sh

# Terminal 2: Start escrow service (URLs passed to frontend, not used by backend)
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 \
FIBER_BUYER_RPC_URL=http://localhost:8229 \
cargo run
```

## Architecture Overview

The backend makes **zero** Fiber RPC calls. All Fiber interactions happen in the browser:

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
│  (NodeB)     │                                  │  (NodeA)     │
└──────────────┘                                  └──────────────┘
```

**Key Points:**
- Frontend fetches Fiber RPC URLs from `/api/config`
- Seller's browser creates hold invoices via `new_invoice` JSON-RPC on seller's node
- Buyer's browser pays invoices via `send_payment` JSON-RPC on buyer's node
- Seller's browser settles invoices via `settle_invoice` JSON-RPC on seller's node (using preimage from escrow API)
- Backend only manages order state and preimage storage

## Test Scenarios

---

### Scenario 1: Happy Path (Real Fiber Payment)

**Goal**: Complete purchase flow with actual Fiber Network hold invoice.

#### Prerequisites
- Both Fiber nodes running with funded channel
- Escrow service started with both `FIBER_SELLER_RPC_URL` and `FIBER_BUYER_RPC_URL`

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Open http://localhost:3000 | Web UI loads, fetches `/api/config` for Fiber URLs |
| 2 | alice | Switch to "alice", browse Market | Products visible |
| 3 | alice | Click "Buy Now" on a product | Order created with preimage, status: `waiting_payment` |
| 4 | bob | Switch to "bob", go to "My Orders" | See order as seller, "Create Invoice" button visible |
| 5 | bob | Click "Create Invoice" | Seller's browser calls `new_invoice` on seller's Fiber node, submits invoice string to escrow via `/api/orders/{id}/invoice` |
| 6 | alice | Switch to "alice", go to "My Orders" | Invoice string visible, "Pay" button available |
| 7 | alice | Click "Pay" | Buyer's browser calls `send_payment` on buyer's Fiber node, then notifies escrow via `/api/orders/{id}/pay`. Status: `funded` |
| 8 | bob | Click "Mark Shipped" | Status: `shipped` |
| 9 | alice | Click "Confirm Receipt" | Escrow reveals preimage in order details. Status: `completed` |
| 10 | bob | "Settle Invoice" button appears | Seller's browser calls `settle_invoice` on seller's Fiber node using preimage from order details |

#### Verification

Check that the hold invoice was actually settled:

```bash
# Query seller's node for invoice status
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"get_invoice","params":[{"payment_hash":"<hash>"}]}'
# Expected: status: "paid"
```

---

### Scenario 2: Dispute - Refund to Buyer (Invoice Cancelled)

**Goal**: Verify dispute resolution cancels the hold invoice and refunds buyer.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | alice | Buy and pay for a product | Invoice created on seller's node, status: `funded` |
| 2 | bob | Ship the item | Status: `shipped` |
| 3 | alice | Click "Dispute", reason: "Item not received" | Status: `disputed` |
| 4 | carol | Go to "Arbiter" tab | Dispute visible with reason |
| 5 | carol | Click "Refund Buyer" | Escrow marks order as refunded. Status: `refunded` |
| 6 | bob | "Cancel Invoice" button appears | Seller's browser calls `cancel_invoice` on seller's Fiber node. Buyer's funds are refunded. |

#### Verification

```bash
# Check invoice was cancelled
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"get_invoice","params":[{"payment_hash":"<hash>"}]}'
# Expected: status: "cancelled"
```

---

### Scenario 3: Dispute - Pay Seller (Invoice Settled)

**Goal**: Verify dispute resolution in seller's favor reveals preimage for settlement.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | alice | Buy and pay | Status: `funded` |
| 2 | bob | Ship | Status: `shipped` |
| 3 | alice | Dispute with reason "Wrong color" | Status: `disputed` |
| 4 | carol | Click "Release to Seller" | Escrow reveals preimage in order details. Status: `completed` |
| 5 | bob | "Settle Invoice" button appears | Seller's browser calls `settle_invoice` on seller's Fiber node using preimage from order details |

#### Verification

- Invoice status on seller's node: `paid`
- Order details include revealed `preimage` (0x-prefixed)

---

### Scenario 4: Timeout Auto-Completion

**Goal**: Verify automatic completion when buyer doesn't respond.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | alice | Buy and pay | Status: `funded` |
| 2 | bob | Ship | Status: `shipped` |
| 3 | - | (Do NOT confirm - simulate buyer gone) | Status remains `shipped` |
| 4 | - | Advance time via Arbiter tab or API | Order auto-completes, preimage revealed |
| 5 | bob | "Settle Invoice" button appears | Seller's browser calls `settle_invoice` on seller's Fiber node |

#### Time Simulation (Web UI)

Go to "Arbiter" tab and click "+1 day" or "+1 week" buttons.

#### Time Simulation (API)

```bash
# Simulate 7 days passing
curl -X POST http://localhost:3000/api/system/tick \
  -H "Content-Type: application/json" \
  -d '{"seconds": 604800}'
```

#### Verification

- Order status: `completed`
- Order details include revealed preimage
- Seller can settle invoice on their Fiber node

---

### Scenario 5: Mock Mode Testing

**Goal**: Verify the system works without Fiber nodes.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Start service without Fiber env vars | `/api/config` returns null URLs |
| 2 | alice | Buy a product | Order created, status: `waiting_payment` |
| 3 | bob | Go to "My Orders" | Order visible, no "Create Invoice" Fiber button (mock mode) |
| 4 | alice | Click "Pay" | Backend manages state transition directly. Status: `funded` |
| 5 | bob | Ship, alice confirms | Normal flow completes without Fiber |

---

## CORS Considerations

When the browser calls Fiber nodes directly, CORS restrictions may apply.

**Solutions:**

1. **Fiber node CORS config**: If supported, enable CORS in the Fiber node configuration
2. **Browser extension**: Use a CORS-disabling extension for testing
3. **Local proxy**: Run a simple proxy that adds CORS headers

Example nginx proxy config:
```nginx
server {
    listen 8230;
    location / {
        proxy_pass http://localhost:8229;
        add_header 'Access-Control-Allow-Origin' '*';
        add_header 'Access-Control-Allow-Methods' 'POST, OPTIONS';
        add_header 'Access-Control-Allow-Headers' 'Content-Type';
    }
}
```

---

## API Quick Reference

### Get Fiber Configuration

```bash
curl http://localhost:3000/api/config
# Returns: {"seller_fiber_rpc_url": "http://...", "buyer_fiber_rpc_url": "http://..."}
```

### Create Order (with preimage)

```bash
# Generate preimage (32 bytes hex)
PREIMAGE="0x$(openssl rand -hex 32)"

# Create order
curl -X POST http://localhost:3000/api/orders \
  -H "X-User-Id: <alice-uuid>" \
  -H "Content-Type: application/json" \
  -d "{\"product_id\": \"<product-uuid>\", \"preimage\": \"$PREIMAGE\"}"
```

Response includes:
- `order_id`
- `payment_hash` (0x-prefixed, SHA256 of preimage)

### Submit Invoice (seller)

```bash
curl -X POST http://localhost:3000/api/orders/<order-uuid>/invoice \
  -H "X-User-Id: <bob-uuid>" \
  -H "Content-Type: application/json" \
  -d '{"invoice": "fibt1..."}'
```

### Create Hold Invoice on Seller's Fiber Node (browser does this)

```bash
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "new_invoice",
    "params": [{
      "amount": "0x3e8",
      "currency": "Fibt",
      "payment_hash": "0x<payment_hash>",
      "expiry": "0x3840",
      "final_expiry_delta": "0x927c00",
      "description": "Escrow order payment"
    }]
  }'
```

### Pay Invoice on Buyer's Fiber Node (browser does this)

```bash
curl -X POST http://localhost:8229 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "send_payment",
    "params": [{"invoice": "<invoice_string>"}]
  }'
```

### Notify Payment to Escrow

```bash
curl -X POST http://localhost:3000/api/orders/<order-uuid>/pay \
  -H "X-User-Id: <alice-uuid>"
```

### Get Order Details (includes preimage when completed)

```bash
curl http://localhost:3000/api/orders/<order-uuid> \
  -H "X-User-Id: <alice-uuid>"
```

### Settle Invoice on Seller's Fiber Node (browser does this)

```bash
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "settle_invoice",
    "params": [{
      "payment_hash": "0x<payment_hash>",
      "payment_preimage": "0x<preimage>"
    }]
  }'
```

### Confirm Order (reveals preimage)

```bash
curl -X POST http://localhost:3000/api/orders/<order-uuid>/confirm \
  -H "X-User-Id: <alice-uuid>" \
  -H "Content-Type: application/json" \
  -d '{}'
```

---

## Checklist

### Mock Mode Testing
- [ ] Scenario 1: Happy path works without Fiber nodes
- [ ] Scenario 2: Dispute refund works
- [ ] Scenario 3: Dispute pay seller works
- [ ] Scenario 4: Timeout auto-completion works

### Real Fiber Mode Testing
- [ ] Fiber nodes started and channel funded
- [ ] `/api/config` returns correct Fiber RPC URLs
- [ ] Seller's browser creates hold invoice on seller's node
- [ ] Seller submits invoice string to escrow
- [ ] Buyer's browser pays invoice on buyer's node
- [ ] Buyer notifies escrow of payment
- [ ] Escrow reveals preimage on confirm/timeout/dispute-to-seller
- [ ] Seller's browser settles invoice on seller's node
- [ ] Seller's browser cancels invoice on dispute-to-buyer
