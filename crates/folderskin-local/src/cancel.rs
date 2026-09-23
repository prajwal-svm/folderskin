//! Stopping a job part-way: a download keeps what it has for next time, and a painting's runtime
//! is ended.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared between whoever may stop a job and the job itself. Clones are the same token.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Asks the job to stop. Safe from any thread, and from a signal handler: it only stores.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// `Err(cancelled)` once the token has been cancelled, for `?` between the steps of a job.
    pub fn check(&self) -> Result<(), crate::Error> {
        if self.is_cancelled() {
            Err(crate::Error::cancelled())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_one_flag() {
        let token = CancelToken::new();
        let other = token.clone();
        assert!(token.check().is_ok());
        other.cancel();
        assert!(token.is_cancelled());
        assert!(token.check().unwrap_err().is_cancelled());
    }
}
