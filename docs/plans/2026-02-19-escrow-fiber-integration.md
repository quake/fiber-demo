# Escrow Fiber Integration - Implementation Complete

**Status:** ✅ Completed

**Goal:** Integrate real Fiber Network hold invoice flow into escrow service.

**Architecture:** Buyer-holds-preimage model - buyer generates preimage, escrow/seller create hold invoice using payment_hash, buyer reveals preimage on confirmation to settle.

---

## Final Architecture

| Step | Actor | Action |
|------|-------|--------|
| 1 | Buyer | Generates preimage locally, computes payment_hash |
| 2 | Buyer | Creates order with payment_hash → Escrow creates hold invoice on seller's Fiber node |
| 3 | Buyer | Pays the hold invoice (funds locked on seller's node) |
| 4 | Seller | Ships item → status = Shipped |
| 5 | Buyer | Confirms receipt, sends preimage to escrow |
| 6 | Escrow | Calls settle_invoice with preimage → Seller receives funds |

### Key Design Decisions

1. **Buyer generates preimage** - Buyer has cryptographic control over payment release
2. **Escrow calls seller's Fiber node** - Uses `FIBER_SELLER_RPC_URL` environment variable
3. **Trust-minimized** - Buyer can refuse to reveal preimage if goods not received
4. **Timeout handling** - Auto-confirm after timeout (preimage must be stored locally by buyer)

---

## Implementation Summary

### Backend Changes

| File | Changes |
|------|---------|
| `fiber-core/src/lib.rs` | Export `RpcFiberClient` |
| `fiber-core/src/crypto/payment.rs` | Add `to_hex()`, `from_hex()` methods |
| `fiber-core/src/fiber/rpc.rs` | New file - RPC client for Fiber node |
| `fiber-escrow-service/src/models.rs` | Remove preimage from Order, add revealed_preimage |
| `fiber-escrow-service/src/state.rs` | Add optional `RpcFiberClient`, new methods for preimage handling |
| `fiber-escrow-service/src/handlers.rs` | Integrate Fiber RPC calls in create_order/confirm_order |
| `fiber-escrow-service/src/main.rs` | Read `FIBER_SELLER_RPC_URL` env var |

### Frontend Changes

| File | Changes |
|------|---------|
| `static/index.html` | Crypto helpers for preimage generation, localStorage for preimage persistence |

---

## API Changes

### Create Order (POST /api/orders)

**Request:**
```json
{
  "product_id": "uuid",
  "payment_hash": "0x..."  // NEW: buyer-provided payment_hash
}
```

**Response:**
```json
{
  "order_id": "uuid",
  "payment_hash": "0x...",
  "amount_shannons": 1000,
  "expires_at": "2026-02-20T...",
  "invoice_string": "fibt1..."  // NEW: if Fiber enabled
}
```

### Confirm Order (POST /api/orders/:id/confirm)

**Request:**
```json
{
  "preimage": "0x..."  // NEW: buyer reveals preimage
}
```

**Response:**
```json
{
  "status": "completed",
  "preimage": "0x..."
}
```

---

## Running the Service

### Mock Mode (Testing)

```bash
cd fiber-escrow/crates/fiber-escrow-service && cargo run
# Open http://localhost:3000
```

### With Real Fiber Node

```bash
# 1. Set up Fiber testnet nodes
./scripts/setup-fiber-testnet.sh

# 2. Run with seller's Fiber node URL
FIBER_SELLER_RPC_URL=http://localhost:8227 cargo run
```

---

## Test Results

All 4 E2E tests pass:
- `test_escrow_happy_path` - Full buyer→pay→ship→confirm flow
- `test_escrow_dispute_refund_to_buyer` - Dispute resolved to buyer
- `test_escrow_dispute_resolved_to_seller` - Dispute resolved to seller (with preimage)
- `test_escrow_order_timeout` - Auto-confirm on timeout

---

## Security Considerations

1. **Preimage Storage** - Buyer's preimage stored in browser localStorage
   - In production, should use more secure storage (e.g., encrypted keystore)

2. **No Preimage = No Payment** - If buyer loses preimage, they cannot confirm receipt
   - Order may auto-confirm on timeout if in Shipped status
   - Dispute can be filed if needed

3. **Fiber Node Trust** - Escrow service needs access to seller's Fiber node RPC
   - In production, seller would run their own node
   - Demo uses single shared node for simplicity
