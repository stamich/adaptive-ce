//! The thread-local last-error message (`docs/NATIVE-INTEROP.md`, rule 2).

use std::cell::RefCell;

use crate::status::{status_of, AceStatus};

/// Block number reported when the last error concerns no block.
pub const NO_BLOCK: u64 = u64::MAX;

struct LastError {
    message: Vec<u8>,
    block: u64,
}

thread_local! {
    static LAST: RefCell<LastError> = const {
        RefCell::new(LastError { message: Vec::new(), block: NO_BLOCK })
    };
}

fn store(message: &str, block: Option<u64>) {
    // `try_with`: during thread teardown the slot may already be gone; the message is then lost,
    // which is better than a panic at the boundary.
    let _ = LAST.try_with(|slot| {
        if let Ok(mut last) = slot.try_borrow_mut() {
            last.message.clear();
            last.message.extend_from_slice(message.as_bytes());
            last.block = block.unwrap_or(NO_BLOCK);
        }
    });
}

/// Records `message` as the last error of this thread and returns `status`.
///
/// Every failing exported function ends with `return fail(...)`.
pub fn fail(status: AceStatus, message: &str) -> AceStatus {
    store(message, None);
    status
}

/// Records a facade error (message and block number) and returns its status code.
pub fn fail_with(error: &ace::Error) -> AceStatus {
    store(&error.to_string(), error.block_id());
    status_of(error.kind())
}

/// Returns a pointer to the UTF-8 message of the last failure on the calling thread.
///
/// The message is not NUL-terminated; its length is [`ace_last_error_len`]. The pointer is valid
/// until the next ACE call on the same thread. It is never null, and the length is 0 when no
/// failure has happened yet.
#[no_mangle]
pub extern "C" fn ace_last_error_ptr() -> *const u8 {
    LAST.try_with(|slot| {
        slot.try_borrow()
            .map_or(std::ptr::null(), |l| l.message.as_ptr())
    })
    .unwrap_or(std::ptr::null())
}

/// Returns the length in bytes of the message at [`ace_last_error_ptr`].
#[no_mangle]
pub extern "C" fn ace_last_error_len() -> usize {
    LAST.try_with(|slot| slot.try_borrow().map_or(0, |l| l.message.len()))
        .unwrap_or(0)
}

/// Returns the block number the last failure concerns (checksum mismatch, block not found), or
/// `UINT64_MAX` when it concerns no block.
#[no_mangle]
pub extern "C" fn ace_last_error_block() -> u64 {
    LAST.try_with(|slot| slot.try_borrow().map_or(NO_BLOCK, |l| l.block))
        .unwrap_or(NO_BLOCK)
}
