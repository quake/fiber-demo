//! Fiber Network client abstraction.

mod mock;
mod traits;

pub use mock::MockFiberClient;
pub use traits::{FiberClient, FiberError, HoldInvoice, PaymentId, PaymentStatus};
