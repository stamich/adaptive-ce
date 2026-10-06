# ACE Format 1.3

Format 1.3 is a minimal additive extension of Format 1.2.

Unchanged:
- file magic and fixed file-header size;
- fixed block-header size;
- dictionary descriptor shape;
- entropy metadata framing;
- AIDX index layout;
- ACET trailer layout;
- CRC32C behavior.

New:
- minor version = 3;
- primary codec ID 3 = `CodecId::Numeric`;
- self-describing `NUM1` numeric codec payload.

Reader compatibility:

| File format | ACE 0.4 reader |
|---|---|
| 1.0 | yes |
| 1.1 | yes |
| 1.2 | yes |
| 1.3 | yes |

A Numeric codec block under a file version <1.3 is rejected as unsupported. This prevents an old
container version from silently acquiring new decoder semantics.
