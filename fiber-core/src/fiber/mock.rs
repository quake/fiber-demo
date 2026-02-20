//! Mock Fiber client for testing.

use super::traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
use crate::crypto::{PaymentHash, Preimage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// State of a mock invoice
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct MockInvoiceState {
    payment_hash: PaymentHash,
    amount: u64,
    status: PaymentStatus,
    created_at: Instant,
    expiry_secs: u64,
}

impl MockInvoiceState {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(self.expiry_secs)
    }
}

/// In-memory mock Fiber client for testing
#[derive(Clone)]
pub struct MockFiberClient {
    /// Map of payment_hash -> invoice state
    invoices: Arc<Mutex<HashMap<PaymentHash, MockInvoiceState>>>,
    /// Map of payment_hash -> preimage (for verification)
    preimages: Arc<Mutex<HashMap<PaymentHash, Preimage>>>,
    /// Simulated balance
    balance: Arc<Mutex<u64>>,
}

impl MockFiberClient {
    /// Create a new mock client with initial balance
    pub fn new(initial_balance: u64) -> Self {
        Self {
            invoices: Arc::new(Mutex::new(HashMap::new())),
            preimages: Arc::new(Mutex::new(HashMap::new())),
            balance: Arc::new(Mutex::new(initial_balance)),
        }
    }

    /// Get current balance
    pub fn balance(&self) -> u64 {
        *self.balance.lock().unwrap()
    }

    /// Register a preimage for an invoice we created
    /// This is called internally when we create an invoice
    pub fn register_preimage(&self, preimage: Preimage) {
        let payment_hash = preimage.payment_hash();
        self.preimages.lock().unwrap().insert(payment_hash, preimage);
    }

    /// Get all invoices (for testing)
    pub fn get_all_invoices(&self) -> Vec<(PaymentHash, PaymentStatus)> {
        self.invoices
            .lock()
            .unwrap()
            .iter()
            .map(|(hash, state)| (*hash, state.status))
            .collect()
    }

    /// Adjust balance by the given amount (can be positive or negative)
    /// Used for settlement simulation
    pub fn adjust_balance(&self, amount: i64) {
        let mut balance = self.balance.lock().unwrap();
        if amount >= 0 {
            *balance = balance.saturating_add(amount as u64);
        } else {
            *balance = balance.saturating_sub((-amount) as u64);
        }
    }
}

impl FiberClient for MockFiberClient {
    async fn create_hold_invoice(
        &self,
        payment_hash: &PaymentHash,
        amount: u64,
        expiry_secs: u64,
    ) -> Result<HoldInvoice, FiberError> {
        let state = MockInvoiceState {
            payment_hash: *payment_hash,
            amount,
            status: PaymentStatus::Pending,
            created_at: Instant::now(),
            expiry_secs,
        };

        self.invoices.lock().unwrap().insert(*payment_hash, state);

        Ok(HoldInvoice {
            payment_hash: *payment_hash,
            amount,
            expiry_secs,
            invoice_string: format!("mock_invoice_{}", hex::encode(payment_hash.as_bytes())),
        })
    }

    async fn pay_hold_invoice(&self, invoice: &HoldInvoice) -> Result<PaymentId, FiberError> {
        // Check balance
        {
            let balance = self.balance.lock().unwrap();
            if *balance < invoice.amount {
                return Err(FiberError::InsufficientFunds);
            }
        }

        // Deduct balance (locked)
        {
            let mut balance = self.balance.lock().unwrap();
            *balance -= invoice.amount;
        }

        // Update invoice status to Held
        {
            let mut invoices = self.invoices.lock().unwrap();
            if let Some(state) = invoices.get_mut(&invoice.payment_hash) {
                if state.is_expired() {
                    // Refund
                    let mut balance = self.balance.lock().unwrap();
                    *balance += invoice.amount;
                    return Err(FiberError::Expired);
                }
                state.status = PaymentStatus::Held;
            } else {
                // Create state for remote invoice
                invoices.insert(
                    invoice.payment_hash,
                    MockInvoiceState {
                        payment_hash: invoice.payment_hash,
                        amount: invoice.amount,
                        status: PaymentStatus::Held,
                        created_at: Instant::now(),
                        expiry_secs: invoice.expiry_secs,
                    },
                );
            }
        }

        Ok(PaymentId::new())
    }

    async fn settle_invoice(
        &self,
        payment_hash: &PaymentHash,
        preimage: &Preimage,
    ) -> Result<(), FiberError> {
        // Verify preimage
        if !payment_hash.verify(preimage) {
            return Err(FiberError::InvalidPreimage);
        }

        let mut invoices = self.invoices.lock().unwrap();
        let state = invoices
            .get_mut(payment_hash)
            .ok_or_else(|| FiberError::InvoiceNotFound(*payment_hash))?;

        match state.status {
            PaymentStatus::Pending => {
                // Can't settle a pending invoice (not paid yet)
                Err(FiberError::PaymentFailed(
                    "Invoice not yet paid".to_string(),
                ))
            }
            PaymentStatus::Held => {
                // Add funds to our balance (we're the receiver settling)
                let mut balance = self.balance.lock().unwrap();
                *balance += state.amount;
                state.status = PaymentStatus::Settled;
                Ok(())
            }
            PaymentStatus::Settled => Err(FiberError::AlreadySettled),
            PaymentStatus::Cancelled => Err(FiberError::AlreadyCancelled),
        }
    }

    async fn cancel_invoice(&self, payment_hash: &PaymentHash) -> Result<(), FiberError> {
        let mut invoices = self.invoices.lock().unwrap();
        let state = invoices
            .get_mut(payment_hash)
            .ok_or_else(|| FiberError::InvoiceNotFound(*payment_hash))?;

        match state.status {
            PaymentStatus::Pending | PaymentStatus::Held => {
                // Refund is handled by the payer side
                state.status = PaymentStatus::Cancelled;
                Ok(())
            }
            PaymentStatus::Settled => Err(FiberError::AlreadySettled),
            PaymentStatus::Cancelled => Err(FiberError::AlreadyCancelled),
        }
    }

    async fn get_payment_status(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<PaymentStatus, FiberError> {
        let invoices = self.invoices.lock().unwrap();
        let state = invoices
            .get(payment_hash)
            .ok_or_else(|| FiberError::InvoiceNotFound(*payment_hash))?;

        if state.is_expired() && state.status == PaymentStatus::Pending {
            return Ok(PaymentStatus::Cancelled);
        }

        Ok(state.status)
    }

    async fn get_balance(&self) -> Result<u64, FiberError> {
        Ok(self.balance())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hold_invoice_lifecycle() {
        let client = MockFiberClient::new(10000);

        // Create a preimage and invoice
        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        assert_eq!(invoice.amount_sat, 1000);

        // Check status is Pending
        let status = client.get_payment_status(&payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Pending);
    }

    #[tokio::test]
    async fn test_pay_hold_invoice() {
        let client = MockFiberClient::new(10000);

        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        // Pay the invoice
        client.pay_hold_invoice(&invoice).await.unwrap();

        // Balance should be reduced
        assert_eq!(client.balance(), 9000);

        // Status should be Held
        let status = client.get_payment_status(&payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Held);
    }

    #[tokio::test]
    async fn test_settle_with_correct_preimage() {
        let client = MockFiberClient::new(10000);

        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        // Pay the invoice
        client.pay_hold_invoice(&invoice).await.unwrap();
        assert_eq!(client.balance(), 9000);

        // Settle with correct preimage
        client.settle_invoice(&payment_hash, &preimage).await.unwrap();

        // Balance should be restored (as receiver)
        assert_eq!(client.balance(), 10000);

        // Status should be Settled
        let status = client.get_payment_status(&payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Settled);
    }

    #[tokio::test]
    async fn test_settle_with_wrong_preimage_fails() {
        let client = MockFiberClient::new(10000);

        let preimage = Preimage::random();
        let wrong_preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        client.pay_hold_invoice(&invoice).await.unwrap();

        // Try to settle with wrong preimage
        let result = client.settle_invoice(&payment_hash, &wrong_preimage).await;
        assert!(matches!(result, Err(FiberError::InvalidPreimage)));
    }

    #[tokio::test]
    async fn test_cancel_invoice() {
        let client = MockFiberClient::new(10000);

        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        client.pay_hold_invoice(&invoice).await.unwrap();
        assert_eq!(client.balance(), 9000);

        // Cancel the invoice
        client.cancel_invoice(&payment_hash).await.unwrap();

        // Status should be Cancelled
        let status = client.get_payment_status(&payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_insufficient_funds() {
        let client = MockFiberClient::new(500);

        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        // Try to pay with insufficient funds
        let result = client.pay_hold_invoice(&invoice).await;
        assert!(matches!(result, Err(FiberError::InsufficientFunds)));
    }

    #[tokio::test]
    async fn test_double_settle_fails() {
        let client = MockFiberClient::new(10000);

        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let invoice = client
            .create_hold_invoice(&payment_hash, 1000, 3600)
            .await
            .unwrap();

        client.pay_hold_invoice(&invoice).await.unwrap();
        client.settle_invoice(&payment_hash, &preimage).await.unwrap();

        // Try to settle again
        let result = client.settle_invoice(&payment_hash, &preimage).await;
        assert!(matches!(result, Err(FiberError::AlreadySettled)));
    }
}
