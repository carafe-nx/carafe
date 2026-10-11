//! Asking long work to stop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ports::AdapterError;

/// A request to stop long work, shared between the one who asks and the work that checks it.
///
/// Clones see the same request: [`CancelToken::cancel`] on any clone is seen by all of them.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Creates a token nobody has cancelled yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the work to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Returns whether the work was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Returns an error if the work was asked to stop.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Cancelled`] once [`CancelToken::cancel`] was called on this token or a clone of it.
    pub fn check(&self) -> Result<(), AdapterError> {
        if self.is_cancelled() {
            Err(AdapterError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_token_is_not_cancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        assert_eq!(token.check(), Ok(()));
    }

    #[test]
    fn a_clone_sees_the_cancel() {
        let token = CancelToken::new();
        let work = token.clone();
        token.cancel();
        assert!(work.is_cancelled());
        assert_eq!(work.check(), Err(AdapterError::Cancelled));
    }

    #[test]
    fn cancelling_twice_stays_cancelled() {
        let token = CancelToken::new();
        token.cancel();
        token.cancel();
        assert_eq!(token.check(), Err(AdapterError::Cancelled));
    }
}
