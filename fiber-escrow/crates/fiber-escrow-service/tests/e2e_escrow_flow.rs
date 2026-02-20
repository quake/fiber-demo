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

/// Generate a random preimage and compute its payment_hash
fn generate_preimage_and_hash() -> (String, String) {
    use fiber_core::Preimage;

    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();

    (preimage.to_hex(), payment_hash.to_hex())
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

    // Get pre-registered user IDs
    let seller_id = get_user_id_by_username(&client, "seller");
    let buyer_id = get_user_id_by_username(&client, "buyer");
    println!("Seller ID: {}, Buyer ID: {}", seller_id, buyer_id);

    let seller_client = EscrowClient::new(&base_url).with_user(&seller_id);
    let buyer_client = EscrowClient::new(&base_url).with_user(&buyer_id);

    // 1. Seller creates a product
    let create_product_resp: serde_json::Value = seller_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Test Widget",
            "description": "A wonderful test widget",
            "price_shannons": 1000
        }))
        .send()
        .expect("Failed to create product")
        .json()
        .expect("Failed to parse create product response");

    let product_id = create_product_resp["product_id"]
        .as_str()
        .expect("No product_id in response");
    println!("Created product: {}", product_id);

    // 2. Buyer generates preimage and payment_hash, then creates order
    let (buyer_preimage, buyer_payment_hash) = generate_preimage_and_hash();
    println!(
        "Buyer's preimage: {}, payment_hash: {}",
        buyer_preimage, buyer_payment_hash
    );

    let create_order_resp: serde_json::Value = buyer_client
        .post("/api/orders")
        .json(&serde_json::json!({
            "product_id": product_id,
            "preimage": buyer_preimage
        }))
        .send()
        .expect("Failed to create order")
        .json()
        .expect("Failed to parse create order response");

    let order_id = create_order_resp["order_id"]
        .as_str()
        .expect("No order_id in response");
    let payment_hash = create_order_resp["payment_hash"]
        .as_str()
        .expect("No payment_hash in response");
    let amount_shannons = create_order_resp["amount_shannons"].as_u64().unwrap();
    println!(
        "Created order: {}, payment_hash: {}, amount: {} shannons",
        order_id, payment_hash, amount_shannons
    );

    // 3. Seller submits invoice (using payment_hash to create it)
    let invoice_string = format!("test_invoice_{}", payment_hash);
    let submit_invoice_resp: serde_json::Value = seller_client
        .post(&format!("/api/orders/{}/invoice", order_id))
        .json(&serde_json::json!({
            "invoice": invoice_string
        }))
        .send()
        .expect("Failed to submit invoice")
        .json()
        .expect("Failed to parse submit invoice response");

    assert_eq!(
        submit_invoice_resp["status"].as_str(),
        Some("invoice_submitted")
    );
    println!("Invoice submitted: {}", invoice_string);

    // 4. Buyer gets order details and sees invoice_string
    let order_details: serde_json::Value = buyer_client
        .get(&format!("/api/orders/{}", order_id))
        .send()
        .expect("Failed to get order details")
        .json()
        .expect("Failed to parse order details");

    assert_eq!(
        order_details["invoice_string"].as_str(),
        Some(invoice_string.as_str())
    );
    println!(
        "Buyer sees invoice: {}",
        order_details["invoice_string"].as_str().unwrap()
    );

    // 5. Buyer pays for the order (notifies payment done)
    let pay_resp: serde_json::Value = buyer_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .expect("Failed to pay order")
        .json()
        .expect("Failed to parse pay response");

    assert_eq!(pay_resp["status"].as_str(), Some("funded"));
    println!("Order funded");

    // 6. Seller ships the order
    let ship_resp: serde_json::Value = seller_client
        .post(&format!("/api/orders/{}/ship", order_id))
        .send()
        .expect("Failed to ship order")
        .json()
        .expect("Failed to parse ship response");

    assert_eq!(ship_resp["status"].as_str(), Some("shipped"));
    println!("Order shipped");

    // 7. Buyer confirms receipt (preimage already stored in escrow)
    let confirm_resp: serde_json::Value = buyer_client
        .post(&format!("/api/orders/{}/confirm", order_id))
        .json(&serde_json::json!({}))
        .send()
        .expect("Failed to confirm order")
        .json()
        .expect("Failed to parse confirm response");

    assert_eq!(confirm_resp["status"].as_str(), Some("completed"));
    println!("Order completed");

    // 8. Seller gets order details -> sees preimage for settlement
    let seller_order_details: serde_json::Value = seller_client
        .get(&format!("/api/orders/{}", order_id))
        .send()
        .expect("Failed to get order details for seller")
        .json()
        .expect("Failed to parse order details");

    let seller_preimage = seller_order_details["preimage"]
        .as_str()
        .expect("Seller should see preimage after completion");
    // Preimage returned is without 0x prefix
    let buyer_preimage_no_prefix = buyer_preimage.strip_prefix("0x").unwrap_or(&buyer_preimage);
    assert_eq!(seller_preimage, buyer_preimage_no_prefix);
    println!(
        "Seller retrieved preimage for settlement: {}",
        seller_preimage
    );

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

    let seller_id = get_user_id_by_username(&client, "seller");
    let buyer_id = get_user_id_by_username(&client, "buyer");

    let seller_client = EscrowClient::new(&base_url).with_user(&seller_id);
    let buyer_client = EscrowClient::new(&base_url).with_user(&buyer_id);

    // 1. Seller creates a product
    let create_product_resp: serde_json::Value = seller_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Disputed Widget",
            "description": "Will be disputed",
            "price_shannons": 500
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let product_id = create_product_resp["product_id"].as_str().unwrap();

    // 2. Buyer generates preimage and creates order with preimage
    let (buyer_preimage, _buyer_payment_hash) = generate_preimage_and_hash();

    let create_order_resp: serde_json::Value = buyer_client
        .post("/api/orders")
        .json(&serde_json::json!({
            "product_id": product_id,
            "preimage": buyer_preimage
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let order_id = create_order_resp["order_id"].as_str().unwrap();
    let payment_hash = create_order_resp["payment_hash"].as_str().unwrap();
    println!(
        "Created order: {}, payment_hash: {}",
        order_id, payment_hash
    );

    // 3. Seller submits invoice
    let invoice_string = format!("test_invoice_{}", payment_hash);
    let _submit_invoice_resp: serde_json::Value = seller_client
        .post(&format!("/api/orders/{}/invoice", order_id))
        .json(&serde_json::json!({
            "invoice": invoice_string
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    println!("Invoice submitted");

    // 4. Buyer pays for the order
    let _pay_resp: serde_json::Value = buyer_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();
    println!("Order funded");

    // 5. Buyer disputes the order (before shipping)
    let dispute_resp: serde_json::Value = buyer_client
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

    // 6. Check dispute appears in arbiter list
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

    // 7. Arbiter resolves in favor of buyer
    let resolve_resp: serde_json::Value = client
        .post(&format!("/api/arbiter/disputes/{}/resolve", order_id))
        .json(&serde_json::json!({ "resolution": "buyer" }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    assert_eq!(resolve_resp["status"].as_str(), Some("resolved"));
    assert_eq!(resolve_resp["resolution"].as_str(), Some("buyer"));
    // Preimage should NOT be revealed when resolved to buyer (payment expires/refunds)
    assert!(
        resolve_resp["preimage"].is_null(),
        "Preimage should be null when resolved to buyer"
    );
    println!(
        "Dispute resolved in favor of buyer, preimage: {:?}",
        resolve_resp["preimage"]
    );

    println!("Test passed: Dispute refund to buyer flow completed successfully");
}

/// Test dispute resolution to seller: buyer reveals preimage to arbiter
#[test]
fn test_escrow_dispute_resolved_to_seller() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const PORT: u16 = 15003;
    let base_url = format!("http://localhost:{}", PORT);

    // Start escrow service
    let service = ServiceProcess::start(&workspace_dir, PORT);
    assert!(
        service.wait_for_ready(&format!("{}/api/health", base_url), Duration::from_secs(30)),
        "Escrow service failed to start"
    );

    let client = EscrowClient::new(&base_url);

    let seller_id = get_user_id_by_username(&client, "seller");
    let buyer_id = get_user_id_by_username(&client, "buyer");

    let seller_client = EscrowClient::new(&base_url).with_user(&seller_id);
    let buyer_client = EscrowClient::new(&base_url).with_user(&buyer_id);

    // 1. Seller creates a product
    let create_product_resp: serde_json::Value = seller_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Seller Wins Widget",
            "description": "Dispute will be resolved to seller",
            "price_shannons": 600
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let product_id = create_product_resp["product_id"].as_str().unwrap();

    // 2. Buyer generates preimage and creates order
    let (buyer_preimage, _buyer_payment_hash) = generate_preimage_and_hash();

    let create_order_resp: serde_json::Value = buyer_client
        .post("/api/orders")
        .json(&serde_json::json!({
            "product_id": product_id,
            "preimage": buyer_preimage
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let order_id = create_order_resp["order_id"].as_str().unwrap();
    let payment_hash = create_order_resp["payment_hash"].as_str().unwrap();

    // 3. Seller submits invoice
    let invoice_string = format!("test_invoice_{}", payment_hash);
    seller_client
        .post(&format!("/api/orders/{}/invoice", order_id))
        .json(&serde_json::json!({ "invoice": invoice_string }))
        .send()
        .unwrap();

    // 4. Buyer pays
    buyer_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .unwrap();

    // 5. Seller ships
    seller_client
        .post(&format!("/api/orders/{}/ship", order_id))
        .send()
        .unwrap();

    // 6. Buyer disputes (maybe unreasonably)
    buyer_client
        .post(&format!("/api/orders/{}/dispute", order_id))
        .json(&serde_json::json!({ "reason": "Item not as described" }))
        .send()
        .unwrap();

    // 7. Try to confirm disputed order (should fail)
    // In escrow-holds-preimage model, preimage is already stored, but confirm fails
    // because order is in Disputed state, not Shipped
    let confirm_resp: serde_json::Value = buyer_client
        .post(&format!("/api/orders/{}/confirm", order_id))
        .json(&serde_json::json!({}))
        .send()
        .unwrap()
        .json()
        .unwrap();

    // confirm_order fails because order is Disputed, not Shipped
    // This is expected behavior
    assert!(
        confirm_resp.get("error").is_some(),
        "Should fail to confirm disputed order"
    );
    println!("Cannot confirm disputed order (expected)");

    // 8. Arbiter resolves to seller
    // In escrow-holds-preimage model, preimage is always available for settlement
    let resolve_resp: serde_json::Value = client
        .post(&format!("/api/arbiter/disputes/{}/resolve", order_id))
        .json(&serde_json::json!({ "resolution": "seller" }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    assert_eq!(resolve_resp["status"].as_str(), Some("resolved"));
    assert_eq!(resolve_resp["resolution"].as_str(), Some("seller"));

    // In escrow-holds-preimage model, preimage is available from escrow storage
    let resolved_preimage = resolve_resp["preimage"]
        .as_str()
        .expect("Preimage should be available for seller resolution");
    let buyer_preimage_no_prefix = buyer_preimage.strip_prefix("0x").unwrap_or(&buyer_preimage);
    assert_eq!(resolved_preimage, buyer_preimage_no_prefix);
    println!(
        "Dispute resolved to seller, preimage: {}",
        resolved_preimage
    );

    println!("Test passed: Dispute resolved to seller with preimage from escrow");
}

/// Test timeout/expiry flow - in escrow-holds-preimage model, timeout without confirmation
/// means escrow auto-settles using stored preimage. This favors the seller who shipped.
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

    let seller_id = get_user_id_by_username(&client, "seller");
    let buyer_id = get_user_id_by_username(&client, "buyer");

    let seller_client = EscrowClient::new(&base_url).with_user(&seller_id);
    let buyer_client = EscrowClient::new(&base_url).with_user(&buyer_id);

    // 1. Seller creates a product
    let create_product_resp: serde_json::Value = seller_client
        .post("/api/products")
        .json(&serde_json::json!({
            "title": "Timeout Widget",
            "description": "Will timeout",
            "price_shannons": 750
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let product_id = create_product_resp["product_id"].as_str().unwrap();

    // 2. Buyer generates preimage and creates order
    let (buyer_preimage, _buyer_payment_hash) = generate_preimage_and_hash();

    let create_order_resp: serde_json::Value = buyer_client
        .post("/api/orders")
        .json(&serde_json::json!({
            "product_id": product_id,
            "preimage": buyer_preimage
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();

    let order_id = create_order_resp["order_id"].as_str().unwrap();
    let payment_hash = create_order_resp["payment_hash"].as_str().unwrap();
    println!(
        "Created order: {}, payment_hash: {}",
        order_id, payment_hash
    );

    // 3. Seller submits invoice
    let invoice_string = format!("test_invoice_{}", payment_hash);
    let _submit_invoice_resp: serde_json::Value = seller_client
        .post(&format!("/api/orders/{}/invoice", order_id))
        .json(&serde_json::json!({
            "invoice": invoice_string
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    println!("Invoice submitted");

    // 4. Buyer pays for the order
    let _pay_resp: serde_json::Value = buyer_client
        .post(&format!("/api/orders/{}/pay", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();
    println!("Order funded");

    // 5. Seller ships the order
    let _ship_resp: serde_json::Value = seller_client
        .post(&format!("/api/orders/{}/ship", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();
    println!("Order shipped. Buyer does not confirm, waiting for timeout...");

    // 6. Advance time past expiry (orders expire after 24 hours by default)
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

    // The shipped order should have timed out
    assert!(
        expired_orders
            .iter()
            .any(|id| id.as_str() == Some(order_id)),
        "Order should be in expired list"
    );

    // 7. Check order status
    let seller_order_details: serde_json::Value = seller_client
        .get(&format!("/api/orders/{}", order_id))
        .send()
        .unwrap()
        .json()
        .unwrap();

    assert_eq!(seller_order_details["status"].as_str(), Some("completed"));

    // In escrow-holds-preimage model, escrow stores preimage at order creation time.
    // On timeout, escrow settles the invoice using the stored preimage.
    let preimage_value = &seller_order_details["preimage"];
    println!(
        "Order timed out. Preimage available to seller: {:?}",
        preimage_value
    );

    // Preimage should be available because escrow holds it from order creation
    let seller_preimage = preimage_value
        .as_str()
        .expect("Preimage should be available after timeout completion");
    let buyer_preimage_no_prefix = buyer_preimage.strip_prefix("0x").unwrap_or(&buyer_preimage);
    assert_eq!(seller_preimage, buyer_preimage_no_prefix);

    println!("Test passed: Order timeout flow - escrow auto-settled with stored preimage");

    // Note: In escrow-holds-preimage model, timeout scenario settles to seller because:
    // 1. Escrow holds the preimage from order creation
    // 2. On timeout (shipped but not confirmed), escrow auto-settles the invoice
    // 3. Seller gets paid, buyer gets the shipped goods
}
