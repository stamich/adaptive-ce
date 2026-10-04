# Dictionary infrastructure

ACE 0.2 introduces stable dictionary identity and resolution boundaries without adding training heuristics yet.

- `DictionaryId` identifies dictionary content in block metadata.
- `DictionaryScope` distinguishes block, segment, file and external lifetimes.
- `DictionaryProvider` decouples the engine from storage and distribution mechanisms.
- `DictionaryRegistry` is a deterministic in-memory reference implementation.

The generic format can carry a dictionary reference, but the 0.2 built-in codecs do not select dictionaries automatically. Dictionary training, shared corpus dictionaries and GraphNet/AdaptiveDB semantic dictionary hints are later milestones.
