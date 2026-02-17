# Fiber Escrow Design

## Overview

基于 Fiber Network Hold Invoice 的托管交易系统。仲裁者（Arbiter）持有 preimage，买家资金锁定在 hold invoice 中，确认履约后 preimage 释放给卖家完成收款。

## 应用场景

通用商品交易 - 买卖双方交易实物/虚拟商品，买家确认收货后放款，仲裁者仅在有争议时介入。

## Key Decisions

| Decision | Choice |
|----------|--------|
| 确认机制 | 买家确认优先，仲裁者处理争议 |
| 超时处理 | 超时自动确认，保护卖家利益 |
| 争议处理 | 简单二选一（全额放款或全额退款） |
| Preimage 管理 | 仲裁者生成并持有 |
| 项目结构 | 共享 fiber-core，独立 fiber-escrow |
| UI 设计 | 单服务多角色（适合演示） |

---

## Architecture

```
fiber-demo/
├── fiber-core/                    # 共享库
│   └── src/
│       ├── fiber/                 # FiberClient trait + MockFiberClient
│       ├── crypto/                # Preimage, PaymentHash
│       └── types/                 # 通用类型
├── fiber-game/                    # 游戏 demo（已有）
│   └── crates/
│       ├── fiber-game-core/       # 游戏专用逻辑
│       ├── fiber-game-oracle/
│       └── fiber-game-player/
└── fiber-escrow/                  # 托管 demo（新建）
    └── crates/
        └── fiber-escrow-service/  # 单服务，多角色 UI
            ├── src/
            │   └── main.rs
            └── static/
                └── index.html
```

---

## Data Model

```rust
/// 用户
struct User {
    id: UserId,
    username: String,
    balance_sat: i64,  // 可为负（演示用）
}

/// 商品
struct Product {
    id: ProductId,
    seller_id: UserId,
    title: String,
    description: String,
    price_sat: u64,
    status: ProductStatus,
}

enum ProductStatus {
    Available,
    Sold,
}

/// 订单
struct Order {
    id: OrderId,
    product_id: ProductId,
    seller_id: UserId,
    buyer_id: UserId,
    amount_sat: u64,
    
    // Arbiter 生成
    preimage: Preimage,
    payment_hash: PaymentHash,
    
    // 状态流转
    status: OrderStatus,
    created_at: Timestamp,
    expires_at: Timestamp,      // 超时自动确认时间
    
    // 争议
    dispute: Option<Dispute>,
}

enum OrderStatus {
    WaitingPayment,    // 等待买家付款
    Funded,            // 资金已锁定（hold invoice paid）
    Shipped,           // 卖家已发货
    Completed,         // 买家确认/超时，preimage 已释放
    Disputed,          // 争议中
    Refunded,          // 仲裁者裁定退款
}

struct Dispute {
    reason: String,
    created_at: Timestamp,
    resolution: Option<DisputeResolution>,
}

enum DisputeResolution {
    ToSeller,  // 放款给卖家
    ToBuyer,   // 退款给买家
}
```

---

## API Endpoints

### 用户管理

```
POST /api/user/register
     Body: { username }
     Response: { user_id, username }

GET  /api/user/me
     Header: X-User-Id: <user_id>
     Response: { user_id, username, balance_sat }
```

### 商品

```
POST /api/products
     Body: { title, description, price_sat }
     Response: { product_id }

GET  /api/products
     Response: { products: [...] }  // 所有 Available 商品

GET  /api/products/mine
     Response: { products: [...] }  // 当前用户创建的商品
```

### 订单

```
POST /api/orders
     Body: { product_id }
     Response: { order_id, payment_hash, amount_sat, expires_at }

GET  /api/orders/mine
     Response: { orders: [...] }  // 作为买家或卖家的订单

POST /api/orders/{id}/pay
     Response: { status }
     Effect: 模拟 hold invoice 付款，锁定资金

POST /api/orders/{id}/ship
     Response: { status }
     Effect: 卖家标记已发货

POST /api/orders/{id}/confirm
     Response: { status, preimage }
     Effect: 释放 preimage，卖家收款

POST /api/orders/{id}/dispute
     Body: { reason }
     Response: { status }
```

### 仲裁者

```
GET  /api/arbiter/disputes
     Response: { disputes: [...] }

POST /api/arbiter/disputes/{order_id}/resolve
     Body: { resolution: "seller" | "buyer" }
     Response: { status }
```

### 系统

```
POST /api/system/tick
     Body: { seconds: 3600 }  // 模拟时间流逝
     Response: { expired_orders: [...] }
     Effect: 检查超时订单，自动确认
```

---

## Web UI

单页面，顶部切换用户身份，Tab 切换功能模块。

```
┌─────────────────────────────────────────────────────────────┐
│  Fiber Escrow Demo                [用户: alice ▼] [余额: 5000]│
├─────────────────────────────────────────────────────────────┤
│  [商品市场]  [我的订单]  [我的商品]  [仲裁台]                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ═══ 商品市场 ═══                                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ iPhone 15    │  │ MacBook Pro  │  │ AirPods      │       │
│  │ 卖家: bob    │  │ 卖家: carol  │  │ 卖家: bob    │       │
│  │ 1000 sats    │  │ 5000 sats    │  │ 500 sats     │       │
│  │ [购买]       │  │ [购买]       │  │ [购买]       │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                             │
│  ═══ 我的订单 ═══                                            │
│  | 商品      | 金额  | 状态     | 操作                    |  │
│  | iPhone 15 | 1000  | 已发货   | [确认收货] [争议]       |  │
│  | AirPods   | 500   | 等待付款 | [付款]                  |  │
│                                                             │
│  ═══ 我的商品 ═══                                            │
│  [+ 创建商品]                                                │
│  | 商品      | 价格  | 状态     |                         |  │
│  | USB Cable | 100   | Available|                         |  │
│                                                             │
│  ═══ 仲裁台 ═══                                              │
│  | 订单 | 买家  | 卖家 | 原因       | 操作                 |  │
│  | #123 | alice | bob  | 未收到货物 | [放款卖家] [退款买家]|  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### UI 功能

1. **用户切换** - 下拉选择用户（alice/bob/carol/arbiter）
2. **商品市场** - 浏览所有可购买商品，点击购买
3. **我的订单** - 查看作为买家的订单，可付款/确认收货/争议
4. **我的商品** - 查看作为卖家的商品和相关订单，可发货
5. **仲裁台** - 查看所有争议，裁决放款或退款

---

## Core Flow

### 正常流程

```
Buyer                    Service                   Seller
  │                         │                         │
  │                    [Seller 创建商品]               │
  │                         │<──POST /products────────│
  │                         │                         │
  │──GET /products─────────>│                         │
  │<───[商品列表]───────────│                         │
  │                         │                         │
  │──POST /orders──────────>│                         │
  │   {product_id}          │                         │
  │                   [生成 preimage]                 │
  │                   [计算 payment_hash]             │
  │<──{order_id, payment_hash}                        │
  │                         │                         │
  │──POST /orders/{id}/pay─>│                         │
  │   [模拟: 创建 hold      │                         │
  │    invoice 并锁定资金]  │                         │
  │                   [status: Funded]                │
  │                         │                         │
  │                         │<──POST /orders/{id}/ship│
  │                   [status: Shipped]               │
  │                         │                         │
  │──POST /orders/{id}/confirm──>│                    │
  │                   [释放 preimage 给卖家]          │
  │                   [卖家 settle invoice]           │
  │                   [status: Completed]             │
  │   [-1000 sats]          │──[+1000 sats]──────────>│
```

### 超时自动确认

```
Buyer                    Service                   Seller
  │                         │                         │
  │   [订单状态: Shipped]   │                         │
  │                         │                         │
  │   [买家未操作...]       │                         │
  │                         │                         │
  │                   [POST /system/tick]             │
  │                   [检测到订单超时]                │
  │                   [自动释放 preimage]             │
  │                   [status: Completed]             │
  │   [-1000 sats]          │──[+1000 sats]──────────>│
```

### 争议流程

```
Buyer                    Service                   Arbiter
  │                         │                         │
  │──POST /orders/{id}/dispute──>│                    │
  │   {reason: "未收到货物"}│                         │
  │                   [status: Disputed]              │
  │                         │                         │
  │                         │<──GET /arbiter/disputes─│
  │                         │───[争议列表]───────────>│
  │                         │                         │
  │                         │<──POST /resolve─────────│
  │                         │   {resolution: "buyer"} │
  │                   [取消 hold invoice]             │
  │                   [status: Refunded]              │
  │   [+退款]               │                         │
```

---

## Implementation Plan

### Phase 1: 项目结构重构

1. 创建 `fiber-core/` 共享库
2. 从 `fiber-game-core` 提取通用代码：
   - `fiber/` (FiberClient trait, MockFiberClient)
   - `crypto/` (Preimage, PaymentHash)
3. 更新 `fiber-game` 依赖 `fiber-core`

### Phase 2: Escrow 服务实现

1. 创建 `fiber-escrow/` workspace
2. 实现数据模型 (User, Product, Order)
3. 实现 API 端点
4. 实现超时检查逻辑

### Phase 3: Web UI

1. 实现单页面 UI
2. 用户切换功能
3. 各 Tab 功能实现
4. 实时状态更新（polling）

### Phase 4: 测试

1. 单元测试
2. E2E 测试文档
3. 手动演示验证

---

## Demo Script

演示脚本，用于展示完整流程：

1. **启动服务**: `cargo run --bin fiber-escrow-service`
2. **切换到 bob** → 创建商品 "iPhone 15" (1000 sats)
3. **切换到 alice** → 浏览市场 → 购买 iPhone → 付款
4. **切换到 bob** → 查看订单 → 点击发货
5. **切换到 alice** → 确认收货
6. **验证余额**: alice -1000, bob +1000

争议演示：
1. 重复上述 1-4 步
2. **alice** 发起争议: "未收到货物"
3. **切换到 arbiter** → 查看争议 → 裁决退款给买家
4. **验证余额**: alice 退款，bob 未收到款

---

## Next Steps

1. 确认设计无误
2. 创建实现计划
3. 开始编码
