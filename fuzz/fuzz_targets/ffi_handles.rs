#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The input is a script of C ABI calls (create engines, compress, decompress, open readers,
    // read ranges, release handles, pass invalid arguments). No call may report a panic or an
    // internal error, and every handle is released.
    ace_ffi::fuzzing::handles(data);
});
