//! `AceSchema`: the record layout of columnar blocks, ABI 1.1 (`docs/FORMAT-1.6.md`).

use ace::{Column, Schema};

use crate::args::input_bytes;
use crate::guard::{guard, guard_value};
use crate::handle::{clear_out, publish, release};
use crate::last_error::{fail, fail_with};
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT};

/// Column kind of [`AceColumn::kind`]: opaque bytes of any width.
pub const ACE_COLUMN_BYTES: u32 = 0;
/// Column kind: unsigned little-endian integers of width 1, 2, 4 or 8.
pub const ACE_COLUMN_UNSIGNED: u32 = 1;
/// Column kind: signed little-endian integers of width 1, 2, 4 or 8.
pub const ACE_COLUMN_SIGNED: u32 = 2;
/// Column kind: IEEE-754 little-endian floats of width 4 or 8.
pub const ACE_COLUMN_FLOAT: u32 = 3;

/// One field of a fixed-size record.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AceColumn {
    /// `ACE_COLUMN_BYTES`, `ACE_COLUMN_UNSIGNED`, `ACE_COLUMN_SIGNED` or `ACE_COLUMN_FLOAT`.
    pub kind: u32,
    /// Bytes per record, 1 to 65 535 (1, 2, 4 or 8 for integers; 4 or 8 for floats).
    pub width: u32,
}

/// The record layout of a table. The type is opaque in C; release it with [`ace_schema_free`].
pub struct AceSchema {
    pub(crate) schema: Schema,
}

/// Creates a schema from `count` columns at `columns`, in record order.
///
/// A record is the concatenation of its columns. Fails with `ACE_ERR_INVALID_ARGUMENT` for an
/// empty list, an unknown kind, a width the kind does not allow or a record above the format's
/// limit. On failure `*out` is null.
///
/// # Safety
///
/// `columns` must be valid for reads of `count` elements; `out` valid for a pointer write.
#[no_mangle]
pub unsafe extern "C" fn ace_schema_new(
    columns: *const AceColumn,
    count: usize,
    out: *mut *mut AceSchema,
) -> AceStatus {
    guard(|| {
        // SAFETY: `out` is writable by the caller's contract.
        if let Err(status) = unsafe { clear_out(out) } {
            return status;
        }
        // SAFETY: `AceColumn` is plain data; the byte view is only used to bounds-check the array.
        let raw = match unsafe {
            input_bytes(
                columns.cast::<u8>(),
                count.saturating_mul(std::mem::size_of::<AceColumn>()),
            )
        } {
            Ok(raw) => raw,
            Err(status) => return status,
        };
        let mut list = Vec::new();
        if list.try_reserve_exact(count).is_err() {
            return fail(ACE_ERR_INVALID_ARGUMENT, "column count is too large");
        }
        for index in 0..count {
            // SAFETY: `raw` covers `count` elements, so element `index` is in bounds.
            let column = unsafe { columns.add(index).read_unaligned() };
            let Ok(width) = u16::try_from(column.width) else {
                return fail(ACE_ERR_INVALID_ARGUMENT, "column width does not fit in 16 bits");
            };
            list.push(match column.kind {
                ACE_COLUMN_BYTES => Column::bytes(width),
                ACE_COLUMN_UNSIGNED => Column::unsigned(width),
                ACE_COLUMN_SIGNED => Column::signed(width),
                ACE_COLUMN_FLOAT => Column::float(width),
                other => {
                    return fail(ACE_ERR_INVALID_ARGUMENT, &format!("unknown column kind {other}"))
                }
            });
        }
        let _ = raw;
        match Schema::new(list) {
            // SAFETY: `out` was validated above.
            Ok(schema) => unsafe { publish(out, AceSchema { schema }) },
            Err(error) => fail_with(&error),
        }
    })
}

/// Returns the size of one record in bytes, or 0 when `schema` is null.
///
/// # Safety
///
/// `schema` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn ace_schema_record_size(schema: *const AceSchema) -> u32 {
    guard_value(0, || {
        // SAFETY: null or live by the caller's contract.
        unsafe { schema.as_ref() }.map_or(0, |handle| handle.schema.record_size())
    })
}

/// Releases a schema. Accepts null. Engines created with it keep their own copy.
///
/// # Safety
///
/// `schema` must be null or a live handle, not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ace_schema_free(schema: *mut AceSchema) {
    guard_value((), || {
        // SAFETY: null or live and not used again by the caller's contract.
        unsafe { release(schema) };
    });
}
