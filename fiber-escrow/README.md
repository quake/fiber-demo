# Fiber Escrow

An escrow trading system built on Fiber Network using hold invoices for secure buyer-seller transactions.

## Overview

This demo implements a marketplace where:

- **Sellers** list products for sale
- **Buyers** purchase with funds locked in hold invoices
- **Arbiter** holds the preimage and resolves disputes
- **Automatic timeout** protects sellers from unresponsive buyers

## How It Works

### Normal Flow

```
Buyer                    Service                   Seller
  │                         │                         │
  │                    [Seller lists product]        │
  │                         │<────────────────────────│
  │                         │                         │
  │──[Buy product]─────────>│                         │
  │                   [Generate preimage]             │
  │<──[order_id, payment_hash]                        │
  │                         │                         │
  │──[Pay hold invoice]────>│                         │
  │                   [Funds locked]                  │
  │                         │                         │
  │                         │<────[Mark shipped]──────│
  │                         │                         │
  │──[Confirm receipt]─────>│                         │
  │                   [Release preimage]              │
  │   [-1000 sats]          │──[+1000 sats]──────────>│
```

### Dispute Flow

If the buyer disputes, the arbiter reviews and decides:
- **Pay Seller**: Release preimage, seller receives funds
- **Refund Buyer**: Cancel invoice, buyer gets refund

### Timeout Protection

If the buyer doesn't confirm within the timeout period, the order auto-completes to protect the seller.

## Running the Demo

```bash
# Start the service
cargo run

# Open in browser
open http://localhost:3000
```

## Web UI

The demo includes a multi-role web interface:

```
┌─────────────────────────────────────────────────────────────┐
│  Fiber Escrow Demo              [User: alice ▼] [Balance]   │
├─────────────────────────────────────────────────────────────┤
│  [Market]  [My Orders]  [My Products]  [Arbiter]            │
└─────────────────────────────────────────────────────────────┘
```

### Demo Users

| User | Role | Starting Balance |
|------|------|------------------|
| alice | Buyer | 10,000 sats |
| bob | Seller | 10,000 sats |
| carol | Seller | 10,000 sats |

### Tabs

- **Market**: Browse and buy available products
- **My Orders**: View orders as buyer (pay, confirm, dispute)
- **My Products**: View products as seller, manage orders (ship)
- **Arbiter**: Resolve disputes (pay seller or refund buyer)

## API Endpoints

### User Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/user/register` | Register new user |
| GET | `/api/user/me` | Get current user info |

### Products

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/products` | List available products |
| POST | `/api/products` | Create a product |
| GET | `/api/products/mine` | List my products |

### Orders

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/orders` | Create order (buy product) |
| GET | `/api/orders/mine` | List my orders |
| POST | `/api/orders/{id}/pay` | Pay for order |
| POST | `/api/orders/{id}/ship` | Mark as shipped |
| POST | `/api/orders/{id}/confirm` | Confirm receipt |
| POST | `/api/orders/{id}/dispute` | Open dispute |

### Arbiter

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/arbiter/disputes` | List open disputes |
| POST | `/api/arbiter/disputes/{id}/resolve` | Resolve dispute |

### System

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/system/tick` | Simulate time passage |

## Demo Script

### Happy Path

1. Start service: `cargo run`
2. Open http://localhost:3000
3. **As bob**: Create product "iPhone 15" (1000 sats)
4. **As alice**: Browse market → Buy iPhone → Pay
5. **As bob**: View orders → Click "Ship"
6. **As alice**: Confirm receipt
7. Check balances: alice -1000, bob +1000

### Dispute Path

1. Follow steps 1-5 above
2. **As alice**: Click "Dispute" instead of confirm
3. **As arbiter**: View disputes → Click "Refund Buyer"
4. Check balances: alice refunded, bob unchanged

### Timeout Path

1. Follow steps 1-5 above (don't confirm)
2. **As anyone**: Call `/api/system/tick` with `{"seconds": 86400}`
3. Order auto-completes, seller receives payment

## Dependencies

This crate depends on `fiber-core` for:
- `Preimage` / `PaymentHash` types
- `FiberClient` trait and `MockFiberClient`

## Data Models

### Order Status Flow

```
WaitingPayment ──[pay]──> Funded ──[ship]──> Shipped
                                                │
                            ┌───────────────────┼───────────────────┐
                            │                   │                   │
                            ▼                   ▼                   ▼
                       Completed           Disputed            (timeout)
                     (buyer confirms)    (buyer disputes)   (auto-complete)
                                               │
                                    ┌──────────┴──────────┐
                                    ▼                     ▼
                                Completed              Refunded
                              (arbiter: seller)     (arbiter: buyer)
```

## License

MIT
