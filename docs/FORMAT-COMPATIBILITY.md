# ACE format compatibility — 0.4

ACE 0.4 writes Format 1.3 and reads 1.0/1.1/1.2/1.3.

| Writer/file format | ACE 0.4 reader |
|---|---|
| 1.0 | supported |
| 1.1 | supported |
| 1.2 | supported |
| 1.3 | supported |

Format 1.3 adds only primary codec ID 3 (`Numeric`) plus its self-describing payload. The outer
file/block header sizes, dictionary descriptors, AIDX and ACET remain unchanged.

A Numeric block declared under a minor version below 3 is rejected, preventing accidental use of
new decoder semantics in an older container version.
