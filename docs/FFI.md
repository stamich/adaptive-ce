# C ABI v1 (`include/ace.h`)

Status: ACE 0.7.0, ABI version **1**. The conventions every function follows (status codes,
last error, handles, buffers, panics) are in [NATIVE-INTEROP.md](NATIVE-INTEROP.md). This
document lists the functions, structs and JSON shapes.

`include/ace.h` is generated from `crates/ace-ffi` by cbindgen (`cbindgen.toml` at the
repository root). CI regenerates it and fails when the committed header differs. Do not edit it
by hand.

The library is built as `libace_ffi` (`.so`, `.dylib`, `.dll`) and `libace_ffi.a`.

## Types

```c
typedef int32_t AceStatus;

typedef struct AceEngine      AceEngine;       // compress/decompress; Send + Sync
typedef struct AceBuffer      AceBuffer;       // bytes owned by ACE
typedef struct AceRangeReader AceRangeReader;  // random access into one ACE file

typedef struct AceConfig {
    size_t   struct_size;      // set to sizeof(AceConfig); ace_config_default does it
    uint32_t profile;          // 0 FAST, 1 BALANCED (default), 2 DENSE
    uint64_t block_size;       // bytes; 0 keeps the library default (1 MiB)
    uint64_t threads;          // 0 = one per available core
    uint64_t memory_budget;    // bytes; 0 = no budget
    uint32_t block_size_policy;// 0 fixed (default), 1 auto
    uint32_t access_hint;      // 0 sequential, 1 balanced (default), 2 random access
    uint8_t  write_index;      // non-zero writes the block index (needed for range reads)
    uint8_t  numeric_specialization; // non-zero (default 1)
    uint8_t  float_specialization;   // non-zero (default 1)
    uint8_t  reserved[5];      // must be zero
} AceConfig;

typedef struct AceLimits {
    size_t   struct_size;
    uint64_t max_output_size;
    uint64_t max_block_size;
    uint64_t max_encoded_block_size;
    uint64_t max_transforms;
    uint64_t max_dictionary_size;
    uint64_t max_blocks;
} AceLimits;
```

`struct_size` lets a later release append fields. A function reads only the fields that fit in
the `struct_size` the caller passed, and uses defaults for the rest. A `struct_size` smaller than
the first field is `ACE_ERR_INVALID_ARGUMENT`. A `NULL` config or limits pointer means all
defaults.

```c
typedef int64_t (*AceReadFn)(void *ctx, uint8_t *buf, size_t capacity);
// returns the number of bytes stored in buf (0 = end of input), or a negative value on failure

typedef AceStatus (*AceWriteFn)(void *ctx, const uint8_t *buf, size_t len);
// returns ACE_OK, or any other status to abort
```

## Functions

### Library

| Function | Description |
|---|---|
| `uint32_t ace_abi_version(void)` | `1`. |
| `const char *ace_engine_version(void)` | NUL-terminated crate version, static storage. |
| `const uint8_t *ace_last_error_ptr(void)` | Message of the last failure on this thread. |
| `size_t ace_last_error_len(void)` | Its length in bytes. |
| `uint64_t ace_last_error_block(void)` | Block number, or `UINT64_MAX`. |

### Configuration

| Function | Description |
|---|---|
| `void ace_config_default(AceConfig *out)` | Fills the defaults, including `struct_size`. |
| `void ace_limits_default(AceLimits *out)` | Fills the default decode limits. |

### Engine

| Function | Description |
|---|---|
| `AceStatus ace_engine_new(const AceConfig *config, const AceLimits *limits, AceEngine **out)` | Validates and builds an engine. |
| `void ace_engine_free(AceEngine *engine)` | Releases it; accepts `NULL`. |

An engine is immutable and may be shared by any number of threads.

### One-shot compression and decompression

| Function | Description |
|---|---|
| `AceStatus ace_compress(const AceEngine *e, const uint8_t *in, size_t len, AceBuffer **out, AceBuffer **stats_json)` | Compresses `in`. `stats_json` may be `NULL`; otherwise it receives the [Stats JSON](#stats-json). |
| `AceStatus ace_decompress(const AceEngine *e, const uint8_t *in, size_t len, AceBuffer **out)` | Decompresses a complete ACE file under the engine's limits. |
| `AceStatus ace_explain_json(const AceEngine *e, const uint8_t *in, size_t len, AceBuffer **out)` | Per-block planner report; slow. |

### Streaming

| Function | Description |
|---|---|
| `AceStatus ace_compress_stream(const AceEngine *e, uint64_t size, AceReadFn read, void *read_ctx, AceWriteFn write, void *write_ctx, AceBuffer **stats_json)` | Reads exactly `size` bytes from `read` and writes the ACE file to `write`. `stats_json` may be `NULL`. |
| `AceStatus ace_decompress_stream(const AceEngine *e, AceReadFn read, void *read_ctx, AceWriteFn write, void *write_ctx)` | Decodes block by block; memory use is bounded by the block size, not the file size. |

The callbacks are invoked on the calling thread, from inside the call. A callback must not call into ACE. If a callback fails, the ACE call returns `ACE_ERR_IO` and the last error says which callback.
In Java the callbacks are Panama upcall stubs; the stub lives as long as the call.

`ace_compress_stream` needs the input size up front and a fixed block size policy
(`block_size_policy == 0`), exactly like `Engine::compress_stream`; otherwise it returns
`ACE_ERR_INVALID_ARGUMENT`. Input is read one block per worker at a time, so memory does not
grow with the input. The ACE header is completed at the end, so the compressed file is
assembled in memory and passed to `write` in chunks when the run finishes; memory use is the
size of the output, not of the input. The stats it returns contain only `input_bytes`,
`output_bytes`, `format_minor` and `blocks`; the other counters are `0`. A reader that yields
fewer or more than `size` bytes gives `ACE_ERR_CORRUPT`.

`ace_decompress_stream` holds one block at a time and writes as it goes.

### Random access

| Function | Description |
|---|---|
| `AceStatus ace_range_open(const AceEngine *e, const uint8_t *file, size_t len, AceRangeReader **out)` | Opens an indexed ACE file held in memory. The bytes are copied. |
| `AceStatus ace_range_open_path(const AceEngine *e, const uint8_t *path, size_t path_len, AceRangeReader **out)` | Opens an indexed ACE file on disk (UTF-8 path); blocks are read on demand. |
| `AceStatus ace_range_len(const AceRangeReader *r, uint64_t *out)` | Original size in bytes. |
| `AceStatus ace_range_block_count(const AceRangeReader *r, uint64_t *out)` | Number of blocks. |
| `AceStatus ace_range_read(AceRangeReader *r, uint64_t start, uint64_t end, AceBuffer **out)` | Original bytes `[start, end)`. |
| `AceStatus ace_range_read_block(AceRangeReader *r, uint64_t block, AceBuffer **out)` | One whole block. |
| `void ace_range_free(AceRangeReader *r)` | Releases it; accepts `NULL`. |

A reader keeps a one-block cache, so it is used by one thread at a time. Open one reader per
thread, or lock around it.

### Buffers

| Function | Description |
|---|---|
| `const uint8_t *ace_buffer_data(const AceBuffer *b)` | Never `NULL`; points to a valid address even when the length is 0. |
| `size_t ace_buffer_len(const AceBuffer *b)` | Length in bytes. |
| `void ace_buffer_free(AceBuffer *b)` | Releases it; accepts `NULL`. |

## Stats JSON

`ace_compress` and `ace_compress_stream` return UTF-8 JSON with these members. Consumers ignore
members they do not know.

```json
{
  "input_bytes": 1048576,
  "output_bytes": 212345,
  "format_minor": 5,
  "blocks": 1,
  "raw_blocks": 0,
  "rle_blocks": 0,
  "lz_blocks": 1,
  "numeric_blocks": 0,
  "time_series_blocks": 0,
  "ratio": 4.938,
  "plan_distribution": { "lz+huffman": 1 }
}
```

`ratio` is derived (`input_bytes / output_bytes`, `1.0` for empty output).

## Explain JSON

`ace_explain_json` returns an array, one object per block:

```json
[{ "block_id": 0, "size": 1048576, "route": "generic",
   "selected": "lz_balanced+huffman", "reason": "…",
   "candidates": ["lz_balanced+huffman", "raw"] }]
```

## Error mapping

The `ErrorKind` to `AceStatus` table is in [NATIVE-INTEROP.md](NATIVE-INTEROP.md#1-status-codes).
`ace_range_read` with a reversed range or one past the end returns `ACE_ERR_INVALID_ARGUMENT`.

## Example

```c
#include "ace.h"

AceEngine *engine = NULL;
AceBuffer *packed = NULL;
AceBuffer *stats = NULL;

if (ace_engine_new(NULL, NULL, &engine) != ACE_OK) goto fail;
if (ace_compress(engine, data, data_len, &packed, &stats) != ACE_OK) goto fail;

use(ace_buffer_data(packed), ace_buffer_len(packed));

ace_buffer_free(stats);
ace_buffer_free(packed);
ace_engine_free(engine);
return 0;

fail:
    fwrite(ace_last_error_ptr(), 1, ace_last_error_len(), stderr);
    ...
```

`crates/ace-ffi/tests/c/smoke.c` is a complete program of this shape; CI compiles and runs it.

## Compatibility rules

- ABI v1 functions and status values are never removed or renumbered within ABI v1.
- Adding a function, a status value, or a trailing struct field (behind `struct_size`) is not an
  ABI change.
- Anything else bumps `ace_abi_version()`, and the JVM binding refuses to load a mismatched
  library.
- The bytes produced are governed by the file format ([FORMAT-1.5.md](FORMAT-1.5.md)), not by
  the ABI.
