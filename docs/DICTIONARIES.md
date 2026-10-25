# Dictionaries

A dictionary is a byte string that the LZ codec treats as history that precedes a block, so the
first bytes of the block can already be matches. It pays off for many small, similar inputs
(records, messages, log lines): the keys and boilerplate that every one of them repeats cost
nothing once the dictionary holds them. The wire format is in [`FORMAT-1.6.md`](FORMAT-1.6.md).

## History

* **0.2** – stable identity and resolution boundaries: `DictionaryId`, `DictionaryScope`,
  `DictionaryProvider`, and `DictionaryRegistry` (a deterministic in-memory implementation).
* **0.8.0** – the dictionary is used: a deterministic trainer, LZ with preset history, the
  `ADIC` section of the file and block descriptors that name a dictionary (Format 1.6).

## The pieces (ACE 0.8.0)

| Piece | Where | What it does |
|---|---|---|
| Trainer | `ace_dictionary::train_dictionary` | builds a dictionary of at most 64 KiB from samples |
| LZ with history | `ace_codecs::lz_encode_with_dictionary`, `lz_decode_with_dictionary` | the token stream is unchanged; matches may reach into the dictionary |
| Configuration | `AceConfig::dictionary: Option<EncoderDictionary>` | id, scope and bytes of the dictionary the encoder may use |
| Engine | `ace_engine::AceEngine` | tries the dictionary per block; embeds or references it; resolves it on decode |
| Registry | `AceEngine::with_dictionaries`, `AceIndexedDecoder::with_dictionaries` | supplies `External` dictionaries to a reader |

## Scopes

| Scope | The file contains | A reader needs |
|---|---|---|
| `File` | the dictionary (`ADIC` section after the file header) | nothing: the file is self-contained |
| `External` | only the dictionary id in each block that uses it | the same bytes under the same id, in the registry |

`Block` and `Segment` are reserved; a block that names them is `Malformed`. A file that refers to a
dictionary it does not have fails with `MissingDictionary(id)`, naming the id.

## What the encoder does

For every block (and for every segment and column of a container block) the engine encodes the
block as the planner chose it, then also as *LZ over `dictionary ‖ block`* followed by each
entropy coder that LZ plans use (none, Huffman, rANS). The dictionary form replaces the planned
block when it is smaller, descriptor included, by at least `min_gain_bytes`. Blocks the planner
sends to NUM1 or TS1 are never tried: numeric structure beats text history. This is an
engine-level candidate, not a planner route, so the planner invariants (no trial encodes) hold.

An embedded dictionary costs its own bytes once per file. If the blocks together save no more
than the section costs (`dictionary length + 28` bytes), the input is encoded again without the
dictionary, and a file in which no block uses the dictionary is exactly the file the same input
gives without one: no section, no version bump. With an `External` dictionary there is nothing to
pay for, so any saving counts.

**The default configuration has no dictionary and writes the bytes of 0.7.0.**

## Choosing a dictionary

* Train on samples that look like the data you will compress, not on the data itself: a
  dictionary memorises shared structure, and the fewer samples are identical the better it
  generalises. 100 – 1 000 samples are usually enough.
* Keep an embedded dictionary small (1 – 4 KiB) when files are small; the section is paid once
  per file. Use an external dictionary when you compress many small files and can ship the
  dictionary separately.
* The dictionary is part of the contract of an external file: change its bytes and the id no
  longer refers to the same thing. `dictionary_id_of(bytes)` derives the id from the content so
  that this cannot happen by accident, and a wrong dictionary that is supplied anyway is caught
  by the block checksum rather than producing wrong output.

## Measured

4 000 JSON-like records (503 KB) in 4 KiB blocks, a 1 KiB dictionary trained on 200 other
records:

| Output | Bytes |
|---|---:|
| no dictionary | 87 947 |
| embedded (`File`) | 67 029 (×1.31) |
| external | 65 977 (×1.33) |

Tests: `crates/ace-engine/tests/dictionary.rs`, `crates/ace-codecs/tests/lz_dictionary.rs`,
`crates/ace-dictionary/tests/trainer.rs`.

## From C and Java

The C ABI 1.1 ([FFI.md](FFI.md)) and the Java SDK expose the same three features as the Rust
facade, with the same bytes.

```java
// Train once from typical inputs, keep dictionary.bytes() and dictionary.id() with your data.
Dictionary dictionary = Dictionary.train(samples, 4096);

Config config = Config.builder().externalDictionary(dictionary).build();
try (Engine writer = Engine.create(config);
     Engine reader = Engine.create(Config.defaults(), DecodeLimits.defaults(), List.of(dictionary))) {
    byte[] packed = writer.compress(record);
    byte[] same = reader.decompress(packed);   // Status.UNSUPPORTED if the dictionary is missing
}
```

`Config.Builder.dictionary` embeds the dictionary when it pays, `externalDictionary` stores only
its id. A `Schema` of `Column`s and a `Segmentation` are set the same way on the `Config` and need
nothing on the reading side. In C the same options are the `dictionary`, `schema` and
segmentation members of `AceConfig`, and `ace_engine_new_ex` takes the external dictionaries of a
reader.
