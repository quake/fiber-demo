# Fiber Escrow E2E Test Flow

Manual end-to-end test scenarios for the Fiber Escrow demo with real Fiber Network integration.

## Prerequisites

### Mock Mode (No Fiber Nodes)

For quick testing without Fiber nodes:

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Service running at http://localhost:3000
# Operates in "trust mode" - payments are simulated
```

### Real Fiber Mode (With Fiber Nodes)

For testing with real Fiber Network payments:

```bash
# Terminal 1: Start Fiber testnet nodes
./scripts/setup-fiber-testnet.sh

# Terminal 2: Start escrow service connected to seller's node (NodeA)
cd fiber-escrow/crates/fiber-escrow-service
FIBER_SELLER_RPC_URL=http://localhost:8227 cargo run
```

The Web UI will show a "Your Fiber Node" input field for buyers to enter their node's RPC URL (default: `http://localhost:8229` for NodeB).

## Architecture Overview

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   Buyer's    │         │    Escrow    │         │   Seller's   │
│  Fiber Node  │         │   Service    │         │  Fiber Node  │
│  (NodeB)     │         │              │         │  (NodeA)     │
└──────┬───────┘         └──────┬───────┘         └──────┬───────┘
       │                        │                        │
       │  1. Create order       │  2. Create hold invoice│
       │  (buyer submits        │  ─────────────────────>│
       │   preimage)            │                        │
       │                        │<───── invoice_string ──│
       │                        │                        │
       │  3. send_payment       │                        │
       │  ────────────────────────────────────────────> │
       │  (buyer pays directly from UI)                  │
       │                        │                        │
       │  4. Verify payment     │  5. get_invoice status │
       │  ─────────────────────>│  ─────────────────────>│
       │                        │<───── status: Held ────│
       │                        │                        │
       │                        │  6. settle_invoice     │
       │                        │  (on confirm/timeout)  │
       │                        │  ─────────────────────>│
```

**Key Points:**
- Escrow only connects to **seller's node** (`FIBER_SELLER_RPC_URL`)
- Buyer enters their own node's RPC URL in the Web UI
- Buyer's browser calls their node directly via JSON-RPC `send_payment`
- Escrow holds the preimage and uses it to settle/cancel invoices

## Test Scenarios

---

### Scenario 1: Happy Path (Real Fiber Payment)

**Goal**: Complete purchase flow with actual Fiber Network hold invoice.

#### Prerequisites
- Both Fiber nodes running with funded channel
- Escrow service started with `FIBER_SELLER_RPC_URL=http://localhost:8227`

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Open http://localhost:3000 | Web UI loads |
| 2 | - | Configure "Your Fiber Node" RPC URL | Set to `http://localhost:8229` (buyer's node) |
| 3 | bob | Switch to "bob", go to "My Products" | Products tab visible |
| 4 | bob | Create product "Test Item" (500 sats) | Product appears in market |
| 5 | alice | Switch to "alice", browse Market | "Test Item" visible |
| 6 | alice | Click "Buy Now" on Test Item | Order created with invoice, status: `waiting_payment` |
| 7 | alice | Go to "My Orders", click "Pay Now" | Browser calls buyer's node `send_payment`, then escrow verifies. Status: `funded` |
| 8 | bob | Switch to "bob", go to "My Orders" | See order as seller |
| 9 | bob | Click "Mark Shipped" | Status: `shipped` |
| 10 | alice | Switch to "alice", click "Confirm Receipt" | Escrow settles invoice. Status: `completed` |

#### Verification

Check that the hold invoice was actually settled:

```bash
# Query seller's node for invoice status
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"get_invoice","params":{"payment_hash":"<hash>"}}'
# Expected: status: "paid"
```

---

### Scenario 2: Dispute - Refund to Buyer (Invoice Cancelled)

**Goal**: Verify dispute resolution cancels the hold invoice and refunds buyer.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | carol | Create product "Disputed Item" (1000 sats) | Product in market |
| 2 | alice | Buy and pay for the item | Invoice created on seller's node, status: `funded` |
| 3 | carol | Ship the item | Status: `shipped` |
| 4 | alice | Click "Dispute", reason: "Item not received" | Status: `disputed` |
| 5 | (arbiter) | Go to "Arbiter" tab | Dispute visible with reason |
| 6 | (arbiter) | Click "Refund Buyer" | Escrow calls `cancel_invoice`. Status: `refunded` |

#### Verification

```bash
# Check invoice was cancelled
curl -X POST http://localhost:8227 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"get_invoice","params":{"payment_hash":"<hash>"}}'
# Expected: status: "cancelled"
```

---

### Scenario 3: Dispute - Pay Seller (Invoice Settled)

**Goal**: Verify dispute resolution in seller's favor settles the invoice.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product "Another Item" (750 sats) | Product in market |
| 2 | alice | Buy and pay | Status: `funded` |
| 3 | bob | Ship | Status: `shipped` |
| 4 | alice | Dispute with reason "Wrong color" | Status: `disputed` |
| 5 | (arbiter) | Click "Release to Seller" | Escrow settles invoice with stored preimage. Status: `completed` |

#### Verification

- Invoice status on seller's node: `paid`
- Response includes revealed `preimage`

---

### Scenario 4: Timeout Auto-Completion

**Goal**: Verify automatic settlement when buyer doesn't respond.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product "Timeout Test" (300 sats) | Product in market |
| 2 | alice | Buy and pay | Status: `funded` |
| 3 | bob | Ship | Status: `shipped` |
| 4 | - | (Do NOT confirm - simulate buyer gone) | Status remains `shipped` |
| 5 | - | Advance time via Arbiter tab or API | Order auto-completes, invoice settled |

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
- Invoice settled automatically using escrow-held preimage

---

### Scenario 5: Payment Verification Failure

**Goal**: Verify escrow correctly rejects unverified payments.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product | Product in market |
| 2 | alice | Buy (creates invoice) | Status: `waiting_payment` |
| 3 | alice | Click "Pay Now" WITHOUT actually sending payment | Error: "Payment not received" |
| 4 | alice | Enter wrong Fiber RPC URL, click "Pay Now" | Error: network/RPC error |

---

## CORS Considerations

When the browser calls the buyer's Fiber node directly, CORS restrictions may apply.

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
- `payment_hash` (SHA256 of preimage)
- `invoice_string` (Fiber hold invoice to pay)

### Get Order Details

```bash
curl http://localhost:3000/api/orders/<order-uuid> \
  -H "X-User-Id: <alice-uuid>"
```

### Pay Invoice (via buyer's Fiber node)

```bash
curl -X POST http://localhost:8229 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "send_payment",
    "params": {"invoice": "<invoice_string>"}
  }'
```

### Verify Payment with Escrow

```bash
curl -X POST http://localhost:3000/api/orders/<order-uuid>/pay \
  -H "X-User-Id: <alice-uuid>"
```

This polls the seller's node for invoice status (up to 30 seconds).

### Confirm Order (settles invoice)

```bash
curl -X POST http://localhost:3000/api/orders/<order-uuid>/confirm \
  -H "X-User-Id: <alice-uuid>" \
  -H "Content-Type: application/json" \
  -d '{}'
```

---

## Checklist

### Mock Mode Testing
- [ ] Scenario 1: Happy path works
- [ ] Scenario 2: Dispute refund works
- [ ] Scenario 3: Dispute pay seller works
- [ ] Scenario 4: Timeout auto-completion works

### Real Fiber Mode Testing
- [ ] Fiber nodes started and channel funded
- [ ] Escrow service connected to seller's node
- [ ] Web UI can reach buyer's node (CORS resolved)
- [ ] Hold invoice created on seller's node
- [ ] Payment sent from buyer's node
- [ ] Payment status verified (Held)
- [ ] Invoice settled on confirm
- [ ] Invoice cancelled on refund
- [ ] Auto-settlement on timeout
