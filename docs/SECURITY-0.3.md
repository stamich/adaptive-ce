# ACE 0.3 security and hardening notes

- Decoder limits are checked before large block allocations.
- Format headers, indexes and reconstructed blocks remain CRC32C-protected against accidental corruption.
- CRC32C is not authentication; untrusted transport requiring authenticity must add a cryptographic integrity layer.
- rANS4x validates model size, lane sizes, payload sum and reconstructed lane bounds.
- Streaming uses checked block/index sizes and verifies exact declared input length.
- SIMD unsafe blocks are isolated to `ace-simd` and guarded by runtime feature detection.
- Planner sampling uses checked ranges and no attacker-controlled allocation proportional to candidate count beyond bounded vectors.
- Independent blocks preserve bounded corruption blast radius and indexed recovery behavior.
