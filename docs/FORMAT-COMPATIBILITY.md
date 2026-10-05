# ACE format compatibility

ACE 0.3.1 does not introduce a new wire format.

| Writer format | ACE 0.3.1 reader |
|---|---|
| 1.0 | supported |
| 1.1 | supported |
| 1.2 | supported |

ACE 0.3.1 writes Format 1.2.

Format hardening covers:
- fixed-header checksum validation;
- block descriptor validation;
- block CRC32C validation;
- AIDX entry-count/resource limits;
- strictly increasing block IDs;
- contiguous logical block offsets;
- ACET trailer checksum validation;
- indexed random-access bounds checks.

The decoder must reject malformed or unsupported input with an `AceError`; parser hardening must not
change successful decoding of valid 1.0/1.1/1.2 files.
