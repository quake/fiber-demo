//! Application state management.

use crate::models::*;
use chrono::{DateTime, Utc};
use fiber_core::{FiberClient, RpcFiberClient};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
    /// Fiber client for seller's node (creates invoices, settles payments)
    seller_fiber_client: Option<Arc<RpcFiberClient>>,
    /// Fiber client for buyer's node (for checking real balances)
    buyer_fiber_client: Option<Arc<RpcFiberClient>>,
    /// Buyer's Fiber RPC URL (for sending payments)
    buyer_fiber_rpc_url: Option<String>,
}

struct AppStateInner {
    users: HashMap<UserId, User>,
    products: HashMap<ProductId, Product>,
    orders: HashMap<OrderId, Order>,
    /// Simulated current time (for timeout testing)
    current_time: Option<DateTime<Utc>>,
}

impl AppState {
    /// Create new state without Fiber integration (for testing)
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                users: HashMap::new(),
                products: HashMap::new(),
                orders: HashMap::new(),
                current_time: None,
            })),
            seller_fiber_client: None,
            buyer_fiber_client: None,
            buyer_fiber_rpc_url: None,
        }
    }

    /// Create new state with Fiber clients for real payments
    pub fn with_fiber_clients(
        seller_client: Option<Arc<RpcFiberClient>>,
        buyer_client: Option<Arc<RpcFiberClient>>,
        buyer_rpc_url: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                users: HashMap::new(),
                products: HashMap::new(),
                orders: HashMap::new(),
                current_time: None,
            })),
            seller_fiber_client: seller_client,
            buyer_fiber_client: buyer_client,
            buyer_fiber_rpc_url: buyer_rpc_url,
        }
    }

    /// Get the seller's Fiber client if configured
    pub fn seller_fiber_client(&self) -> Option<&Arc<RpcFiberClient>> {
        self.seller_fiber_client.as_ref()
    }

    /// Get the buyer's Fiber client if configured
    pub fn buyer_fiber_client(&self) -> Option<&Arc<RpcFiberClient>> {
        self.buyer_fiber_client.as_ref()
    }

    /// Get buyer's Fiber RPC URL if configured
    pub fn buyer_fiber_rpc_url(&self) -> Option<&str> {
        self.buyer_fiber_rpc_url.as_deref()
    }

    /// Get current time (real or simulated)
    pub fn now(&self) -> DateTime<Utc> {
        self.inner
            .lock()
            .unwrap()
            .current_time
            .unwrap_or_else(Utc::now)
    }

    /// Advance simulated time by seconds
    pub fn advance_time(&self, seconds: i64) {
        let mut inner = self.inner.lock().unwrap();
        let current = inner.current_time.unwrap_or_else(Utc::now);
        inner.current_time = Some(current + chrono::Duration::seconds(seconds));
    }

    // User operations

    pub fn register_user(&self, username: String) -> User {
        let user = User::new(username);
        let mut inner = self.inner.lock().unwrap();
        inner.users.insert(user.id, user.clone());
        user
    }

    pub async fn get_user(&self, id: UserId) -> Option<User> {
        let mut user = {
            let inner = self.inner.lock().unwrap();
            inner.users.get(&id).cloned()?
        };
        
        let username = user.username.to_lowercase();
        
        // Try to get real balance from Fiber node
        let mut real_balance = None;
        if username == "seller" {
            if let Some(client) = self.seller_fiber_client() {
                if let Ok(bal) = client.get_balance().await {
                    real_balance = Some(bal as i64);
                }
            }
        } else if username == "buyer" {
            if let Some(client) = self.buyer_fiber_client() {
                if let Ok(bal) = client.get_balance().await {
                    real_balance = Some(bal as i64);
                }
            }
        }

        if let Some(bal) = real_balance {
            user.balance_shannons = bal;
        } else {
            // Fallback: Calculate simulated balance based on orders
            let inner = self.inner.lock().unwrap();
            let mut balance: i64 = 0;
            for order in inner.orders.values() {
                if order.seller_id == id && order.status == OrderStatus::Completed {
                    balance += order.amount_shannons as i64;
                }
                if order.buyer_id == id {
                    match order.status {
                        OrderStatus::Funded | OrderStatus::Shipped | OrderStatus::Completed | OrderStatus::Disputed => {
                            balance -= order.amount_shannons as i64;
                        }
                        _ => {}
                    }
                }
            }
            user.balance_shannons = balance;
        }
        
        Some(user)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Option<User> {
        let id = {
            let inner = self.inner.lock().unwrap();
            inner.users.values().find(|u| u.username == username).map(|u| u.id)
        };
        if let Some(id) = id {
            self.get_user(id).await
        } else {
            None
        }
    }

    pub async fn list_users(&self) -> Vec<User> {
        let ids: Vec<UserId> = {
            let inner = self.inner.lock().unwrap();
            inner.users.keys().cloned().collect()
        };
        
        let mut users = Vec::new();
        for id in ids {
            if let Some(user) = self.get_user(id).await {
                users.push(user);
            }
        }
        users
    }

    // Product operations

    pub fn create_product(
        &self,
        seller_id: UserId,
        title: String,
        description: String,
        price_shannons: u64,
    ) -> Product {
        let product = Product::new(seller_id, title, description, price_shannons);
        let mut inner = self.inner.lock().unwrap();
        inner.products.insert(product.id, product.clone());
        product
    }

    pub fn get_product(&self, id: ProductId) -> Option<Product> {
        self.inner.lock().unwrap().products.get(&id).cloned()
    }

    pub fn list_available_products(&self) -> Vec<Product> {
        self.inner
            .lock()
            .unwrap()
            .products
            .values()
            .filter(|p| p.status == ProductStatus::Available)
            .cloned()
            .collect()
    }

    pub fn list_products_by_seller(&self, seller_id: UserId) -> Vec<Product> {
        self.inner
            .lock()
            .unwrap()
            .products
            .values()
            .filter(|p| p.seller_id == seller_id)
            .cloned()
            .collect()
    }

    // Order operations

    pub fn create_order(
        &self,
        product: &Product,
        buyer_id: UserId,
        payment_hash: fiber_core::PaymentHash,
    ) -> Order {
        let order = Order::new(product, buyer_id, payment_hash, 24); // 24 hour timeout
        let mut inner = self.inner.lock().unwrap();
        inner.orders.insert(order.id, order.clone());
        order
    }

    pub fn get_order(&self, id: OrderId) -> Option<Order> {
        self.inner.lock().unwrap().orders.get(&id).cloned()
    }

    pub fn update_order_status(&self, id: OrderId, status: OrderStatus) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&id) {
            order.status = status;
        }
    }

    pub fn list_orders_for_user(&self, user_id: UserId) -> Vec<Order> {
        self.inner
            .lock()
            .unwrap()
            .orders
            .values()
            .filter(|o| o.buyer_id == user_id || o.seller_id == user_id)
            .cloned()
            .collect()
    }

    pub fn list_disputed_orders(&self) -> Vec<Order> {
        self.inner
            .lock()
            .unwrap()
            .orders
            .values()
            .filter(|o| o.status == OrderStatus::Disputed)
            .cloned()
            .collect()
    }

    pub fn add_dispute(&self, order_id: OrderId, reason: String) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&order_id) {
            order.dispute = Some(Dispute {
                reason,
                created_at: Utc::now(),
                resolution: None,
            });
            order.status = OrderStatus::Disputed;
        }
    }

    pub fn resolve_dispute(&self, order_id: OrderId, resolution: DisputeResolution) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&order_id) {
            if let Some(ref mut dispute) = order.dispute {
                dispute.resolution = Some(resolution);
            }
            order.status = match resolution {
                DisputeResolution::ToSeller => OrderStatus::Completed,
                DisputeResolution::ToBuyer => OrderStatus::Refunded,
            };
        }
    }

    /// Check for expired orders and auto-confirm them
    /// Returns tuples of (OrderId, PaymentHash, Preimage) for settlement
    pub fn process_expired_orders(
        &self,
    ) -> Vec<(
        OrderId,
        fiber_core::PaymentHash,
        Option<fiber_core::Preimage>,
    )> {
        let now = self.now();
        let mut expired = Vec::new();

        let mut inner = self.inner.lock().unwrap();
        for order in inner.orders.values_mut() {
            // Only auto-confirm shipped orders that have expired
            if order.status == OrderStatus::Shipped && order.expires_at <= now {
                order.status = OrderStatus::Completed;
                expired.push((
                    order.id,
                    order.payment_hash.clone(),
                    order.revealed_preimage.clone(),
                ));
            }
        }

        expired
    }

    /// Get revealed preimage for a completed order (for settlement)
    pub fn get_revealed_preimage(&self, order_id: OrderId) -> Option<fiber_core::Preimage> {
        let inner = self.inner.lock().unwrap();
        inner
            .orders
            .get(&order_id)
            .and_then(|o| o.revealed_preimage.clone())
    }

    /// Set revealed preimage when buyer confirms receipt
    pub fn set_revealed_preimage(&self, order_id: OrderId, preimage: fiber_core::Preimage) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&order_id) {
            order.revealed_preimage = Some(preimage);
        }
    }

    pub fn set_order_invoice(&self, id: OrderId, invoice: String) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(order) = inner.orders.get_mut(&id) {
            order.invoice_string = Some(invoice);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
