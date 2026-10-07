# ACE 0.4 fuzzing

ACE 0.4 retains all 0.3.1 malformed-container targets and adds:

```bash
cargo fuzz run numeric_decode
cargo fuzz run bitpack_decode
```

`numeric_decode` fuzzes width/mode/bit-width/count/tail/packed-length combinations with bounded
expected output. `bitpack_decode` fuzzes u32/u64 fixed-width decoders across all legal widths.

Hard invariant: arbitrary malformed bytes may return an error, but must not panic, loop forever or
trigger unbounded allocation.
