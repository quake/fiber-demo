//! End-to-end integration tests for the escrow flow.
//!
//! These tests verify the full HTTP interaction for escrow trading.
//!
//! Run with: cargo test --test e2e_escrow_flow -- --nocapture --test-threads=1

use std::process::{Child, Command};
use std::time::Duration;

/// Helper to start the escrow service process
struct ServiceProcess {
    child: Child,
    name: String,
}

impl ServiceProcess {
    fn start(crate_dir: &str, port: u16) -> Self {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "fiber-escrow-service"])
            .current_dir(crate_dir)
            .env("PORT", port.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = cmd.spawn().expect("Failed to start escrow service");

        Self {
            child,
            name: format!("fiber-escrow-service:{}", port),
        }
    }

    fn wait_for_ready(&self, url: &str, timeout: Duration) -> bool {
        let client = reqwest::blocking::Client::new();
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if client.get(url).send().is_ok() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        println!("Stopped {}", self.name);
    }
}

/// Helper struct to manage API calls with user context
struct EscrowClient {
    client: reqwest::blocking::Client,
    base_url: String,
    user_id: Option<String>,
}

impl EscrowClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            base_url: base_url.to_string(),
            user_id: None,
        }
    }

    fn with_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        let mut req = self.client.get(format!("{}{}", self.base_url, path));
        if let Some(ref user_id) = self.user_id {
            req = req.header("X-User-Id", user_id);
        }
        req
    }

    fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        let mut req = self.client.post(format!("{}{}", self.base_url, path));
        if let Some(ref user_id) = self.user_id {
            req = req.header("X-User-Id", user_id);
        }
        req
    }
}

/// Get user ID by username from the users list
fn get_user_id_by_username(client: &EscrowClient, username: &str) -> String {
    let resp: serde_json::Value = client
        .get("/api/users")
        .send()
        .expect("Failed to list users")
        .json()
        .expect("Failed to parse users");

    resp["users"]
        .as_array()
        .expect("users should be array")
        .iter()
        .find(|u| u["username"].as_str() == Some(username))
        .expect(&format!("User {} not found", username))["id"]
        .as_str()
        .expect("user id should be string")
        .to_string()
}

/// Test complete happy path: seller creates product, buyer purchases, seller ships, buyer confirms
#[test]
fn test_escrow_happy_path() {
    // CARGO_MANIFEST_DIR is fiber-escrow-service, go up to fiber-escrow workspace
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const PORT: u16 = 15000;
    let base_url = format!("http://localhost:{}", PORT);

    // Start escrow service
    let service = ServiceProcess::start(&workspace_dir, PORT);
    assert!(
        service.wait_for_ready(&format!("{}/api/health", base_url), Duration::from_secs(30)),
        "Escrow service failed to start"
    );

    let client = EscrowClient::new(&base_url);

    // Get pre-registered user IDs (alice=seller, bob=buyer)
    let alice_id = get_user_id_by_username(&client, "alice");
    let bob_id = get_user_id_by_username(&client, "bob");
    println!("Alice ID: {}, Bob ID: {}", alice_id, bob_id);

    // Check initial balances
    let alice_client = EscrowClient::new(&base_url).with_user(&alice_id);
    let bob_client = EscrowClient::new(&base_url).with_user(&bob_id);

    let alice_info: serde_json::Value = alice_client
        .get("/api/user/me")
        .send()
        .expect("Failed to get alice info")
        .json()
        .expect("Failed to parse alice info");
    let alice_initial_balance = alice_info["balance_sat"].as_i64().unwrap();

    let bob_info: serde_json::Value = bob_client
        .get("/api/user/me")
        .send()
        .expect("Failed to get bob info")
        .json()
        .expect("Failed to parse bob info");
    let bob_initial_balance = bob_info["balance_sat"].as_i64().unwrap();

    println!(
        "Initial balances - Alice: {}, Bob: {}",
        alice_initial_balance, bob_initial_balance
    );

    // 1. Alice creates a product
    let create_product_resp: serde_json::Value = alice_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Test Widget",
            "description": "A wonderful test widget",
            "price_sat": 1000
        }))
        .send()
        .expect("Failed to create product")
        .json()
        .expect("Failed to parse create product response");

    let product_id = create_product_resp["product_id"]
        .as_str()
        .expect("No product_id in response");
    println!("Created product: {}", product_id);

    // 2. Bob creates an order for the product
    let create_order_resp: serde_json::Value = bob_client
        .post("/api/orders")
        .json(&serde_json::json!({
            "product_id": product_id
        }))
        .send()
        .expect("Failed to create order")
        .json()
        .expect("Failed to parse create order response");

    let order_id = create_order_resp["order_id"]
        .as_str()
        .expect("No order_id in response");
    let amount_sat = create_order_resp["amount_sat"].as_u64().unwrap();
    println!("Created order: {}, amount: {} sats", order_id, amount_sat);

    // 3. Bob pays for the order
    let pay_resp: serde_json::Value = bob_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .expect("Failed to pay order")
        .json()
        .expect("Failed to parse pay response");

    assert_eq!(pay_resp["status"].as_str(), Some("funded"));
    println!("Order funded");

    // Verify Bob's balance decreased
    let bob_info_after_pay: serde_json::Value = bob_client
        .get("/api/user/me")
        .send()
        .expect("Failed to get bob info")
        .json()
        .expect("Failed to parse bob info");
    let bob_balance_after_pay = bob_info_after_pay["balance_sat"].as_i64().unwrap();
    assert_eq!(
        bob_balance_after_pay,
        bob_initial_balance - amount_sat as i64
    );
    println!("Bob's balance after pay: {}", bob_balance_after_pay);

    // 4. Alice ships the order
    let ship_resp: serde_json::Value = alice_client
        .post(&format!("/api/orders/{}/ship", order_id))
        .send()
        .expect("Failed to ship order")
        .json()
        .expect("Failed to parse ship response");

    assert_eq!(ship_resp["status"].as_str(), Some("shipped"));
    println!("Order shipped");

    // 5. Bob confirms receipt
    let confirm_resp: serde_json::Value = bob_client
        .post(&format!("/api/orders/{}/confirm", order_id))
        .send()
        .expect("Failed to confirm order")
        .json()
        .expect("Failed to parse confirm response");

    assert_eq!(confirm_resp["status"].as_str(), Some("completed"));
    assert!(confirm_resp["preimage"].as_str().is_some());
    println!(
        "Order completed, preimage: {}",
        confirm_resp["preimage"].as_str().unwrap()
    );

    // 6. Verify final balances
    let alice_info_final: serde_json::Value = alice_client
        .get("/api/user/me")
        .send()
        .expect("Failed to get alice info")
        .json()
        .expect("Failed to parse alice info");
    let alice_final_balance = alice_info_final["balance_sat"].as_i64().unwrap();

    let bob_info_final: serde_json::Value = bob_client
        .get("/api/user/me")
        .send()
        .expect("Failed to get bob info")
        .json()
        .expect("Failed to parse bob info");
    let bob_final_balance = bob_info_final["balance_sat"].as_i64().unwrap();

    println!(
        "Final balances - Alice: {}, Bob: {}",
        alice_final_balance, bob_final_balance
    );

    // Alice should have gained the product price
    assert_eq!(
        alice_final_balance,
        alice_initial_balance + amount_sat as i64
    );
    // Bob should have spent the product price
    assert_eq!(bob_final_balance, bob_initial_balance - amount_sat as i64);

    println!("Test passed: Happy path escrow flow completed successfully");
}

/// Test dispute resolution flow: buyer disputes, arbiter resolves to buyer (refund)
#[test]
fn test_escrow_dispute_refund_to_buyer() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const PORT: u16 = 15001;
    let base_url = format!("http://localhost:{}", PORT);

    // Start escrow service
    let service = ServiceProcess::start(&workspace_dir, PORT);
    assert!(
        service.wait_for_ready(&format!("{}/api/health", base_url), Duration::from_secs(30)),
        "Escrow service failed to start"
    );

    let client = EscrowClient::new(&base_url);

    let alice_id = get_user_id_by_username(&client, "alice");
    let bob_id = get_user_id_by_username(&client, "bob");

    let alice_client = EscrowClient::new(&base_url).with_user(&alice_id);
    let bob_client = EscrowClient::new(&base_url).with_user(&bob_id);

    // Get initial balances
    let bob_info: serde_json::Value = bob_client
        .get("/api/user/me")
        .send()
        .unwrap()
        .json()
        .unwrap();
    let bob_initial_balance = bob_info["balance_sat"].as_i64().unwrap();

    // 1. Alice creates a product
    let create_product_resp: serde_json::Value = alice_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Disputed Widget",
            "description": "Will be disputed",
            "price_sat": 500
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let product_id = create_product_resp["product_id"].as_str().unwrap();

    // 2. Bob creates and pays for an order
    let create_order_resp: serde_json::Value = bob_client
        .post("/api/orders")
        .json(&serde_json::json!({ "product_id": product_id }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let order_id = create_order_resp["order_id"].as_str().unwrap();
    let _amount_sat = create_order_resp["amount_sat"].as_u64().unwrap();

    let _pay_resp: serde_json::Value = bob_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();

    // 3. Bob disputes the order (before shipping)
    let dispute_resp: serde_json::Value = bob_client
        .post(&format!("/api/orders/{}/dispute", order_id))
        .json(&serde_json::json!({
            "reason": "Seller is not responding"
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    assert_eq!(dispute_resp["status"].as_str(), Some("disputed"));
    println!("Order disputed");

    // 4. Check dispute appears in arbiter list
    let disputes: serde_json::Value = client
        .get("/api/arbiter/disputes")
        .send()
        .unwrap()
        .json()
        .unwrap();

    let dispute_list = disputes["disputes"].as_array().unwrap();
    assert!(
        dispute_list
            .iter()
            .any(|d| d["id"].as_str() == Some(order_id)),
        "Disputed order should appear in arbiter list"
    );
    println!("Dispute visible to arbiter");

    // 5. Arbiter resolves in favor of buyer
    let resolve_resp: serde_json::Value = client
        .post(&format!("/api/arbiter/disputes/{}/resolve", order_id))
        .json(&serde_json::json!({ "resolution": "buyer" }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    assert_eq!(resolve_resp["status"].as_str(), Some("resolved"));
    assert_eq!(resolve_resp["resolution"].as_str(), Some("buyer"));
    println!("Dispute resolved in favor of buyer");

    // 6. Verify Bob got refunded
    let bob_info_final: serde_json::Value = bob_client
        .get("/api/user/me")
        .send()
        .unwrap()
        .json()
        .unwrap();
    let bob_final_balance = bob_info_final["balance_sat"].as_i64().unwrap();

    // Bob should be back to initial balance (refunded)
    assert_eq!(bob_final_balance, bob_initial_balance);
    println!(
        "Bob refunded: initial={}, final={}",
        bob_initial_balance, bob_final_balance
    );

    println!("Test passed: Dispute refund to buyer flow completed successfully");
}

/// Test timeout/expiry flow using simulated time
#[test]
fn test_escrow_order_timeout() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const PORT: u16 = 15002;
    let base_url = format!("http://localhost:{}", PORT);

    // Start escrow service
    let service = ServiceProcess::start(&workspace_dir, PORT);
    assert!(
        service.wait_for_ready(&format!("{}/api/health", base_url), Duration::from_secs(30)),
        "Escrow service failed to start"
    );

    let client = EscrowClient::new(&base_url);

    let alice_id = get_user_id_by_username(&client, "alice");
    let bob_id = get_user_id_by_username(&client, "bob");

    let alice_client = EscrowClient::new(&base_url).with_user(&alice_id);
    let bob_client = EscrowClient::new(&base_url).with_user(&bob_id);

    // Get initial balance for Alice
    let alice_info: serde_json::Value = alice_client
        .get("/api/user/me")
        .send()
        .unwrap()
        .json()
        .unwrap();
    let alice_initial_balance = alice_info["balance_sat"].as_i64().unwrap();

    // 1. Alice creates a product
    let create_product_resp: serde_json::Value = alice_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Timeout Widget",
            "description": "Will timeout",
            "price_sat": 750
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let product_id = create_product_resp["product_id"].as_str().unwrap();

    // 2. Bob creates, pays, and Alice ships
    let create_order_resp: serde_json::Value = bob_client
        .post("/api/orders")
        .json(&serde_json::json!({ "product_id": product_id }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let order_id = create_order_resp["order_id"].as_str().unwrap();
    let amount_sat = create_order_resp["amount_sat"].as_u64().unwrap();

    let _pay_resp: serde_json::Value = bob_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let _ship_resp: serde_json::Value = alice_client
        .post(&format!("/api/orders/{}/ship", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();

    println!("Order created, paid, and shipped. Waiting for timeout...");

    // 3. Advance time past expiry (orders expire after 24 hours by default)
    // Advance 25 hours = 25 * 3600 = 90000 seconds
    let tick_resp: serde_json::Value = client
        .post("/api/system/tick")
        .json(&serde_json::json!({ "seconds": 90000 }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let expired_orders = tick_resp["expired_orders"].as_array().unwrap();
    println!("Expired orders: {:?}", expired_orders);

    // The shipped order should have expired and funds released to seller
    assert!(
        expired_orders
            .iter()
            .any(|id| id.as_str() == Some(order_id)),
        "Order should be in expired list"
    );

    // 4. Verify Alice received the funds
    let alice_info_final: serde_json::Value = alice_client
        .get("/api/user/me")
        .send()
        .unwrap()
        .json()
        .unwrap();
    let alice_final_balance = alice_info_final["balance_sat"].as_i64().unwrap();

    assert_eq!(
        alice_final_balance,
        alice_initial_balance + amount_sat as i64
    );
    println!(
        "Alice received funds after timeout: initial={}, final={}",
        alice_initial_balance, alice_final_balance
    );

    println!("Test passed: Order timeout flow completed successfully");
}
