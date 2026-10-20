//! `AceConfig` and `AceLimits`: plain structs that grow behind `struct_size`.

use std::mem::size_of;

use ace::{AccessHint, BlockSizePolicy, Config, DecodeLimits, Profile};

use crate::last_error::fail;
use crate::status::{AceStatus, ACE_ERR_INVALID_ARGUMENT};

/// Compression settings. See `docs/FFI.md`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AceConfig {
    /// `sizeof(AceConfig)` as the caller knows it; `ace_config_default` sets it.
    pub struct_size: usize,
    /// 0 FAST, 1 BALANCED (default), 2 DENSE.
    pub profile: u32,
    /// Block size in bytes; 0 keeps the library default.
    pub block_size: u64,
    /// Worker threads; 0 means one per available core.
    pub threads: u64,
    /// Soft memory cap in bytes; 0 keeps the library default.
    pub memory_budget: u64,
    /// 0 fixed (default), 1 auto.
    pub block_size_policy: u32,
    /// 0 sequential, 1 balanced (default), 2 random access.
    pub access_hint: u32,
    /// Non-zero writes the block index, which range reads need.
    pub write_index: u8,
    /// Non-zero enables the integer pipelines.
    pub numeric_specialization: u8,
    /// Non-zero enables the floating-point pipelines.
    pub float_specialization: u8,
    /// Must be zero.
    pub reserved: [u8; 5],
}

/// Decode limits. A field of 0 keeps the library default. See `docs/FFI.md`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AceLimits {
    /// `sizeof(AceLimits)` as the caller knows it; `ace_limits_default` sets it.
    pub struct_size: usize,
    /// Maximum total decoded output in bytes.
    pub max_output_size: u64,
    /// Maximum decoded size of one block in bytes.
    pub max_block_size: u64,
    /// Maximum encoded size of one block in bytes.
    pub max_encoded_block_size: u64,
    /// Maximum number of transforms of one block.
    pub max_transforms: u64,
    /// Maximum embedded dictionary size in bytes.
    pub max_dictionary_size: u64,
    /// Maximum number of blocks in a file.
    pub max_blocks: u64,
}

impl AceConfig {
    fn defaults() -> Self {
        Self {
            struct_size: size_of::<Self>(),
            profile: 1,
            block_size: Config::default().block_size() as u64,
            threads: 0,
            memory_budget: 0,
            block_size_policy: 0,
            access_hint: 1,
            write_index: 1,
            numeric_specialization: 1,
            float_specialization: 1,
            reserved: [0; 5],
        }
    }
}

impl AceLimits {
    fn defaults() -> Self {
        let limits = DecodeLimits::default();
        Self {
            struct_size: size_of::<Self>(),
            max_output_size: limits.max_output_size(),
            max_block_size: limits.max_block_size() as u64,
            max_encoded_block_size: limits.max_encoded_block_size() as u64,
            max_transforms: limits.max_transforms() as u64,
            max_dictionary_size: limits.max_dictionary_size() as u64,
            max_blocks: limits.max_blocks() as u64,
        }
    }
}

/// Fills `out` with the default settings, including `struct_size`. Does nothing for null.
///
/// # Safety
///
/// `out` must be null or valid for a write of one `AceConfig`.
#[no_mangle]
pub unsafe extern "C" fn ace_config_default(out: *mut AceConfig) {
    if !out.is_null() {
        // SAFETY: non-null and writable by the caller's contract.
        unsafe { out.write(AceConfig::defaults()) };
    }
}

/// Fills `out` with the default decode limits, including `struct_size`. Does nothing for null.
///
/// # Safety
///
/// `out` must be null or valid for a write of one `AceLimits`.
#[no_mangle]
pub unsafe extern "C" fn ace_limits_default(out: *mut AceLimits) {
    if !out.is_null() {
        // SAFETY: non-null and writable by the caller's contract.
        unsafe { out.write(AceLimits::defaults()) };
    }
}

/// Copies the part of `*source` that both sides know over `target`.
///
/// # Safety
///
/// `source` must be valid for reads of `min(struct_size, size_of::<T>())` bytes.
unsafe fn overlay<T: Copy>(source: *const T, struct_size: usize, target: &mut T) {
    let known = struct_size.min(size_of::<T>());
    // SAFETY: both pointers are valid for `known` bytes and cannot overlap (`target` is a local).
    unsafe {
        std::ptr::copy_nonoverlapping(
            source.cast::<u8>(),
            std::ptr::from_mut(target).cast::<u8>(),
            known,
        );
    }
}

fn invalid(message: &str) -> AceStatus {
    fail(ACE_ERR_INVALID_ARGUMENT, message)
}

fn to_usize(value: u64, name: &str) -> Result<usize, AceStatus> {
    usize::try_from(value).map_err(|_| invalid(&format!("{name} does not fit in size_t")))
}

/// Reads the `struct_size` field of a caller struct.
///
/// # Safety
///
/// `source` must be valid for a read of one `usize`.
unsafe fn declared_size(source: *const usize, name: &str) -> Result<usize, AceStatus> {
    // SAFETY: `struct_size` is the first field of both structs; readable by the caller's contract.
    let declared = unsafe { source.read_unaligned() };
    if declared < size_of::<usize>() {
        return Err(invalid(&format!(
            "{name}.struct_size is smaller than its first field"
        )));
    }
    Ok(declared)
}

/// Converts the caller's `AceConfig` (null = defaults) into a validated facade [`Config`].
///
/// # Errors
///
/// [`ACE_ERR_INVALID_ARGUMENT`] for a bad `struct_size`, an unknown enumeration value, a non-zero
/// reserved byte or a value the engine rejects.
///
/// # Safety
///
/// `config` must be null or point to a readable struct of at least `struct_size` bytes.
pub unsafe fn config_from_raw(config: *const AceConfig) -> Result<Config, AceStatus> {
    let mut value = AceConfig::defaults();
    if !config.is_null() {
        // SAFETY: non-null; `struct_size` is the first field.
        let declared = unsafe { declared_size(config.cast::<usize>(), "AceConfig") }?;
        // SAFETY: the caller guarantees `declared` readable bytes.
        unsafe { overlay(config, declared, &mut value) };
    }
    if value.reserved != [0; 5] {
        return Err(invalid("AceConfig.reserved must be zero"));
    }
    let profile = match value.profile {
        0 => Profile::Fast,
        1 => Profile::Balanced,
        2 => Profile::Dense,
        other => return Err(invalid(&format!("unknown profile {other}"))),
    };
    let policy = match value.block_size_policy {
        0 => BlockSizePolicy::Fixed,
        1 => BlockSizePolicy::Auto,
        other => return Err(invalid(&format!("unknown block_size_policy {other}"))),
    };
    let hint = match value.access_hint {
        0 => AccessHint::Sequential,
        1 => AccessHint::Balanced,
        2 => AccessHint::RandomAccess,
        other => return Err(invalid(&format!("unknown access_hint {other}"))),
    };
    let mut builder = Config::builder()
        .profile(profile)
        .threads(to_usize(value.threads, "threads")?)
        .write_index(value.write_index != 0)
        .block_size_policy(policy)
        .access_hint(hint)
        .numeric_specialization(value.numeric_specialization != 0)
        .float_specialization(value.float_specialization != 0);
    if value.block_size != 0 {
        builder = builder.block_size(to_usize(value.block_size, "block_size")?);
    }
    if value.memory_budget != 0 {
        builder = builder.memory_budget(to_usize(value.memory_budget, "memory_budget")?);
    }
    builder
        .build()
        .map_err(|error| crate::last_error::fail_with(&error))
}

/// Converts the caller's `AceLimits` (null = defaults) into facade [`DecodeLimits`].
///
/// # Errors
///
/// [`ACE_ERR_INVALID_ARGUMENT`] for a bad `struct_size` or a value that does not fit `size_t`.
///
/// # Safety
///
/// `limits` must be null or point to a readable struct of at least `struct_size` bytes.
pub unsafe fn limits_from_raw(limits: *const AceLimits) -> Result<DecodeLimits, AceStatus> {
    let mut value = AceLimits {
        struct_size: size_of::<AceLimits>(),
        max_output_size: 0,
        max_block_size: 0,
        max_encoded_block_size: 0,
        max_transforms: 0,
        max_dictionary_size: 0,
        max_blocks: 0,
    };
    if !limits.is_null() {
        // SAFETY: non-null; `struct_size` is the first field.
        let declared = unsafe { declared_size(limits.cast::<usize>(), "AceLimits") }?;
        // SAFETY: the caller guarantees `declared` readable bytes.
        unsafe { overlay(limits, declared, &mut value) };
    }
    let mut builder = DecodeLimits::builder();
    if value.max_output_size != 0 {
        builder = builder.max_output_size(value.max_output_size);
    }
    if value.max_block_size != 0 {
        builder = builder.max_block_size(to_usize(value.max_block_size, "max_block_size")?);
    }
    if value.max_encoded_block_size != 0 {
        builder = builder.max_encoded_block_size(to_usize(
            value.max_encoded_block_size,
            "max_encoded_block_size",
        )?);
    }
    if value.max_transforms != 0 {
        builder = builder.max_transforms(to_usize(value.max_transforms, "max_transforms")?);
    }
    if value.max_dictionary_size != 0 {
        builder = builder
            .max_dictionary_size(to_usize(value.max_dictionary_size, "max_dictionary_size")?);
    }
    if value.max_blocks != 0 {
        builder = builder.max_blocks(to_usize(value.max_blocks, "max_blocks")?);
    }
    Ok(builder.build())
}
