//! Fiber Network client abstraction.

mod mock;
mod rpc;
mod traits;

pub use mock::MockFiberClient;
pub use rpc::{CkbInvoiceStatus, Currency, RpcFiberClient};
pub use traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
