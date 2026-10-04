# ACE fuzz targets

Install `cargo-fuzz`, then run for example:

```bash
cargo fuzz run ace_decoder
cargo fuzz run rans_decoder
```

Targets are intentionally outside the normal workspace build and exercise bounded malformed-input behavior.
