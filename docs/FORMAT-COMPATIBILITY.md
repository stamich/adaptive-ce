# ACE format compatibility — 0.5

ACE 0.5 reads Format 1.0 – 1.4 and writes the **minimal** version a file needs: 1.3 unless the
file contains a TS1 block, then 1.4 ([`FORMAT-1.4.md`](FORMAT-1.4.md)).

| File format | ACE 0.4.x reader | ACE 0.5 reader |
|---|---|---|
| 1.0 | supported | supported |
| 1.1 | supported | supported |
| 1.2 | supported | supported |
| 1.3 | supported | supported |
| 1.4 | rejected at the file header (`UnsupportedVersion`) | supported |

| Version | Adds |
|---|---|
| 1.1 | dictionary descriptors |
| 1.2 | rANS4x entropy coder, block index (AIDX) and trailer (ACET) |
| 1.3 | primary codec 3 `Numeric` (NUM1 payload; u16 lanes since 0.4.5) |
| 1.4 | primary codec 4 `TimeSeries` (TS1 payload: Gorilla f64 / f32, RunDelta) |

Every extension adds only a codec / coder id plus its self-describing payload; outer file and
block header sizes, dictionary descriptors, AIDX and ACET remain unchanged. A block whose
codec is newer than the declared minor version is rejected, preventing accidental use of new
decoder semantics in an older container version.

To produce files for ACE 0.4.x readers with ACE 0.5, compress with `--disable-float`
(`AceConfig::enable_float_specialization = false`): the output is byte-identical to ACE 0.4.6.
Without the flag, data that no TS1 block helps (all of Corpus V3) is also written as 1.3.
