# ACE 0.2 scalar rANS

ACE 0.2 introduces a deterministic scalar 32-bit range Asymmetric Numeral System implementation.

- probability bits: 12,
- normalized total frequency: 4096,
- alphabet: 256 byte symbols,
- state lower bound: `1 << 23`,
- normalized-frequency metadata: 256 little-endian `u16` values.

Normalization is deterministic. Every present symbol receives at least one frequency unit, the final sum is exactly 4096, and tie handling follows symbol order. Runtime planner decisions can therefore remain bit-for-bit reproducible across worker counts.

The 0.2 implementation is intentionally scalar. Interleaved/SIMD rANS is planned for 0.3 after the scalar wire format and malformed-input handling are hardened.
