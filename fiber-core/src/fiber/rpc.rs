//! RPC client for Fiber Network nodes.
//!
//! This module provides a real implementation of `FiberClient` that communicates
//! with a Fiber Network node via JSON-RPC.

use crate::crypto::{PaymentHash, Preimage};
use crate::fiber::traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Currency for Fiber invoices
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Currency {
    /// Mainnet
    Fibb,
    /// Testnet
    Fibt,
    /// Devnet
    Fibd,
}

impl Default for Currency {
    fn default() -> Self {
        Self::Fibt // testnet by default
    }
}

/// Invoice status from Fiber RPC
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum CkbInvoiceStatus {
    /// The invoice is open and can be paid
    Open,
    /// The invoice is cancelled
    Cancelled,
    /// The invoice is expired
    Expired,
    /// The invoice is received, but not settled yet (hold invoice with payment received)
    Received,
    /// The invoice is paid/settled
    Paid,
}

/// RPC client for Fiber Network
pub struct RpcFiberClient {
    /// HTTP client
    client: Client,
    /// Fiber node RPC URL
    rpc_url: String,
    /// Currency to use for invoices
    currency: Currency,
}

impl RpcFiberClient {
    /// Create a new RPC client
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            rpc_url: rpc_url.into(),
            currency: Currency::default(),
        }
    }

    /// Create a new RPC client with specific currency
    pub fn with_currency(rpc_url: impl Into<String>, currency: Currency) -> Self {
        Self {
            client: Client::new(),
            rpc_url: rpc_url.into(),
            currency,
        }
    }

    /// Make a JSON-RPC call
    /// Note: Fiber RPC expects params as an array containing a single object
    async fn call(&self, method: &str, params: Value) -> Result<Value, FiberError> {
        // Wrap params in array as required by Fiber RPC
        let params_array = json!([params]);
        
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params_array
        });

        // Debug: log the request
        println!("[RpcFiberClient] {} -> {}", method, serde_json::to_string(&request).unwrap_or_default());

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FiberError::NetworkError(e.to_string()))?;

        let result: Value = response
            .json()
            .await
            .map_err(|e| FiberError::NetworkError(e.to_string()))?;

        // Debug: log the response
        println!("[RpcFiberClient] {} <- {}", method, serde_json::to_string(&result).unwrap_or_default());

        if let Some(error) = result.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(FiberError::NetworkError(msg.to_string()));
        }

        result
            .get("result")
            .cloned()
            .ok_or_else(|| FiberError::NetworkError("No result in response".to_string()))
    }
}

impl FiberClient for RpcFiberClient {
    /// Create a hold invoice using payment_hash (without preimage)
    async fn create_hold_invoice(
        &self,
        payment_hash: &PaymentHash,
        amount_sat: u64,
        expiry_secs: u64,
    ) -> Result<HoldInvoice, FiberError> {
        // Convert amount from satoshis to shannons (1 sat = 100 shannons in CKB context)
        // Note: Fiber uses shannons as the base unit
        let amount_shannons = amount_sat * 100;

        // final_expiry_delta is in milliseconds
        // Fiber requires minimum of 9,600,000 ms (160 minutes / 2h40m)
        // We use the minimum value to allow faster testing
        let final_expiry_delta_ms: u64 = 9_600_000; // 160 minutes in milliseconds (Fiber minimum)

        let params = json!({
            "amount": format!("0x{:x}", amount_shannons),
            "currency": self.currency,
            "payment_hash": payment_hash.to_hex(),
            "expiry": format!("0x{:x}", expiry_secs),
            "final_expiry_delta": format!("0x{:x}", final_expiry_delta_ms),
            "description": "Fiber Escrow Payment",
        });

        let result = self.call("new_invoice", params).await?;

        let invoice_address = result
            .get("invoice_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FiberError::NetworkError("No invoice_address in response".to_string()))?
            .to_string();

        Ok(HoldInvoice {
            payment_hash: payment_hash.clone(),
            amount_sat,
            expiry_secs,
            invoice_string: invoice_address,
        })
    }

    /// Pay a hold invoice
    ///
    /// This sends a payment to the invoice. For hold invoices, the payment will
    /// be held until the recipient settles or cancels.
    async fn pay_hold_invoice(&self, invoice: &HoldInvoice) -> Result<PaymentId, FiberError> {
        let params = json!({
            "invoice": invoice.invoice_string,
        });

        let result = self.call("send_payment", params).await?;

        // Check payment status
        let status = result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match status {
            "success" | "inflight" | "created" => Ok(PaymentId::new()),
            _ => {
                let error = result
                    .get("failed_error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Payment failed");
                Err(FiberError::PaymentFailed(error.to_string()))
            }
        }
    }

    /// Settle a hold invoice with preimage
    ///
    /// This reveals the preimage to claim the held funds.
    async fn settle_invoice(
        &self,
        payment_hash: &PaymentHash,
        preimage: &Preimage,
    ) -> Result<(), FiberError> {
        // Verify preimage matches payment hash
        if preimage.payment_hash() != *payment_hash {
            return Err(FiberError::InvalidPreimage);
        }

        let params = json!({
            "payment_hash": payment_hash.to_hex(),
            "payment_preimage": preimage.to_hex(),
        });

        self.call("settle_invoice", params).await?;
        Ok(())
    }

    /// Cancel a hold invoice
    ///
    /// This refunds any held funds back to the sender.
    async fn cancel_invoice(&self, payment_hash: &PaymentHash) -> Result<(), FiberError> {
        let params = json!({
            "payment_hash": payment_hash.to_hex(),
        });

        let result = self.call("cancel_invoice", params).await?;

        // Check if cancellation was successful
        let status: Option<CkbInvoiceStatus> = result
            .get("status")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        match status {
            Some(CkbInvoiceStatus::Cancelled) => Ok(()),
            Some(CkbInvoiceStatus::Paid) => Err(FiberError::AlreadySettled),
            _ => Ok(()), // Assume success if status is not explicitly wrong
        }
    }

    /// Get payment/invoice status
    async fn get_payment_status(
        &self,
        payment_hash: &PaymentHash,
    ) -> Result<PaymentStatus, FiberError> {
        let params = json!({
            "payment_hash": payment_hash.to_hex(),
        });

        let result = self.call("get_invoice", params).await?;

        let status: CkbInvoiceStatus = result
            .get("status")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| FiberError::NetworkError("No status in response".to_string()))?;

        Ok(match status {
            CkbInvoiceStatus::Open => PaymentStatus::Pending,
            CkbInvoiceStatus::Received => PaymentStatus::Held,
            CkbInvoiceStatus::Paid => PaymentStatus::Settled,
            CkbInvoiceStatus::Cancelled | CkbInvoiceStatus::Expired => PaymentStatus::Cancelled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_serialization() {
        assert_eq!(
            serde_json::to_string(&Currency::Fibt).unwrap(),
            "\"Fibt\""
        );
        assert_eq!(
            serde_json::to_string(&Currency::Fibb).unwrap(),
            "\"Fibb\""
        );
    }

    #[test]
    fn test_invoice_status_deserialization() {
        let status: CkbInvoiceStatus = serde_json::from_str("\"Open\"").unwrap();
        assert_eq!(status, CkbInvoiceStatus::Open);

        let status: CkbInvoiceStatus = serde_json::from_str("\"Received\"").unwrap();
        assert_eq!(status, CkbInvoiceStatus::Received);
        
        let status: CkbInvoiceStatus = serde_json::from_str("\"Paid\"").unwrap();
        assert_eq!(status, CkbInvoiceStatus::Paid);
    }
}
