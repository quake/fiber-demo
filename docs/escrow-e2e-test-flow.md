# Fiber Escrow E2E Test Flow

Manual end-to-end test scenarios for the Fiber Escrow demo.

## Prerequisites

```bash
cd fiber-escrow && cargo run
# Service running at http://localhost:3000
```

## Test Scenarios

---

### Scenario 1: Happy Path (Normal Purchase)

**Goal**: Verify complete purchase flow from listing to payment completion.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product "Test Item" (500 sats) | Product appears in market |
| 2 | alice | Browse market, click "Buy" on Test Item | Order created, status: `waiting_payment` |
| 3 | alice | Click "Pay" on the order | Status: `funded`, alice balance -500 |
| 4 | bob | Go to "My Products", click "Ship" | Status: `shipped` |
| 5 | alice | Go to "My Orders", click "Confirm" | Status: `completed` |
| 6 | - | Check balances | alice: 9500, bob: 10500 |

#### Verification

```bash
# Check alice balance
curl -H "X-User-Id: <alice-uuid>" http://localhost:3000/api/user/me

# Check bob balance  
curl -H "X-User-Id: <bob-uuid>" http://localhost:3000/api/user/me
```

---

### Scenario 2: Dispute - Refund to Buyer

**Goal**: Verify dispute resolution in favor of buyer.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | carol | Create product "Disputed Item" (1000 sats) | Product appears in market |
| 2 | alice | Buy and pay for the item | Status: `funded` |
| 3 | carol | Ship the item | Status: `shipped` |
| 4 | alice | Click "Dispute", reason: "Item not received" | Status: `disputed` |
| 5 | arbiter | Go to "Arbiter" tab, see dispute | Dispute visible with reason |
| 6 | arbiter | Click "Refund Buyer" | Status: `refunded` |
| 7 | - | Check balances | alice: unchanged, carol: unchanged |

#### Verification

- Alice's balance should return to original (refund)
- Carol's balance unchanged (never received payment)
- Order status is `refunded`

---

### Scenario 3: Dispute - Pay Seller

**Goal**: Verify dispute resolution in favor of seller.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product "Another Item" (750 sats) | Product appears in market |
| 2 | alice | Buy and pay for the item | Status: `funded` |
| 3 | bob | Ship the item | Status: `shipped` |
| 4 | alice | Click "Dispute", reason: "Wrong color" | Status: `disputed` |
| 5 | arbiter | Go to "Arbiter" tab, click "Pay Seller" | Status: `completed` |
| 6 | - | Check balances | alice: -750, bob: +750 |

#### Verification

- Dispute resolved in seller's favor
- Payment completed despite dispute

---

### Scenario 4: Timeout Auto-Completion

**Goal**: Verify automatic completion when buyer doesn't respond.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create product "Timeout Test" (300 sats) | Product in market |
| 2 | alice | Buy and pay | Status: `funded` |
| 3 | bob | Ship the item | Status: `shipped` |
| 4 | - | Do NOT confirm (simulate buyer gone) | Status remains `shipped` |
| 5 | - | Simulate time passage (API call below) | Order auto-completes |
| 6 | - | Check balances | alice: -300, bob: +300 |

#### Time Simulation

```bash
# Simulate 24 hours passing (default timeout)
curl -X POST http://localhost:3000/api/system/tick \
  -H "Content-Type: application/json" \
  -d '{"seconds": 86400}'
```

#### Verification

- Order status changed to `completed` automatically
- Seller received payment without buyer confirmation

---

### Scenario 5: Multiple Concurrent Orders

**Goal**: Verify system handles multiple orders correctly.

#### Steps

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | bob | Create 3 products (100, 200, 300 sats) | 3 products in market |
| 2 | alice | Buy all 3 products | 3 orders created |
| 3 | alice | Pay for all 3 | All status: `funded` |
| 4 | bob | Ship all 3 | All status: `shipped` |
| 5 | alice | Confirm order 1, dispute order 2, ignore order 3 | Mixed statuses |
| 6 | arbiter | Refund order 2 | Order 2: `refunded` |
| 7 | - | Tick time by 24h | Order 3: auto-completed |

#### Final State

- Order 1: `completed` (manual confirm)
- Order 2: `refunded` (dispute resolved)
- Order 3: `completed` (timeout)
- alice: -400 (100 + 300, refunded 200)
- bob: +400

---

### Scenario 6: Edge Cases

#### 6a: Buy Own Product

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | alice | Create a product | Product created |
| 2 | alice | Try to buy own product | Error: "Cannot buy own product" |

#### 6b: Double Payment

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Create and buy a product | Order: `waiting_payment` |
| 2 | - | Pay for the order | Order: `funded` |
| 3 | - | Try to pay again | Error: "Order already paid" |

#### 6c: Confirm Before Ship

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Create, buy, and pay | Order: `funded` |
| 2 | buyer | Try to confirm | Error: "Order not shipped" |

#### 6d: Ship as Buyer

| Step | User | Action | Expected Result |
|------|------|--------|-----------------|
| 1 | - | Create, buy, and pay | Order: `funded` |
| 2 | buyer | Try to ship | Error: "Not the seller" |

---

## API Quick Reference

### Get User ID

Users are pre-registered on startup. Get their IDs:

```bash
# Register returns user_id, or use existing demo users
curl http://localhost:3000/api/products
# Response includes seller_id for each product
```

### Common Headers

```bash
# All authenticated endpoints need X-User-Id header
-H "X-User-Id: <uuid>"
```

### Full API Test

```bash
# 1. List products
curl http://localhost:3000/api/products

# 2. Create product (as bob)
curl -X POST http://localhost:3000/api/products \
  -H "X-User-Id: <bob-uuid>" \
  -H "Content-Type: application/json" \
  -d '{"title": "Test", "description": "A test item", "price_sat": 100}'

# 3. Create order (as alice)
curl -X POST http://localhost:3000/api/orders \
  -H "X-User-Id: <alice-uuid>" \
  -H "Content-Type: application/json" \
  -d '{"product_id": "<product-uuid>"}'

# 4. Pay order
curl -X POST http://localhost:3000/api/orders/<order-uuid>/pay \
  -H "X-User-Id: <alice-uuid>"

# 5. Ship order (as bob)
curl -X POST http://localhost:3000/api/orders/<order-uuid>/ship \
  -H "X-User-Id: <bob-uuid>"

# 6. Confirm order (as alice)
curl -X POST http://localhost:3000/api/orders/<order-uuid>/confirm \
  -H "X-User-Id: <alice-uuid>"
```

---

## Checklist

- [ ] Scenario 1: Happy path works
- [ ] Scenario 2: Dispute refund works
- [ ] Scenario 3: Dispute pay seller works
- [ ] Scenario 4: Timeout auto-completion works
- [ ] Scenario 5: Multiple concurrent orders work
- [ ] Scenario 6a: Cannot buy own product
- [ ] Scenario 6b: Cannot double-pay
- [ ] Scenario 6c: Cannot confirm before ship
- [ ] Scenario 6d: Cannot ship as buyer
