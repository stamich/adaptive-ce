//! Panic containment for every exported function (`docs/NATIVE-INTEROP.md`, rule 7).
//!
//! The three `ace_last_error_*` accessors and the two version functions are not guarded: they are
//! infallible by construction (`try_with` / `try_borrow`, constants), and a guard would overwrite
//! the very message they report.

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::last_error::fail;
use crate::status::{AceStatus, ACE_ERR_PANIC};

/// Text of a panic payload, when it is a string.
fn payload_text(payload: &(dyn Any + Send)) -> &str {
    if let Some(text) = payload.downcast_ref::<&str>() {
        text
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text
    } else {
        "non-string payload"
    }
}

/// Runs `body`; if it panics, records the panic as the last error and returns `on_panic`.
///
/// No Rust panic may unwind into the host: C has no unwinding, and the JVM would crash.
pub fn guard_value<T>(on_panic: T, body: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(payload) => {
            let message = format!("internal panic: {}", payload_text(payload.as_ref()));
            let _ = fail(ACE_ERR_PANIC, &message);
            on_panic
        }
    }
}

/// [`guard_value`] for functions that return a status: a panic becomes [`ACE_ERR_PANIC`].
pub fn guard(body: impl FnOnce() -> AceStatus) -> AceStatus {
    guard_value(ACE_ERR_PANIC, body)
}
