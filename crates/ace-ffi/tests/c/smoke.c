/*
 * Smoke test of the C ABI: build with `tools/ace-check_c0.8.0.sh`, which compiles this file with
 * -Wall -Wextra -Werror against include/ace.h and links libace_ffi.
 */
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ace.h"

/* The JVM bindings hard-code these layouts (jvm/ace-native, AceBindings.java). */
_Static_assert(sizeof(AceConfig) == 56, "AceConfig size");
_Static_assert(offsetof(AceConfig, profile) == 8, "AceConfig.profile");
_Static_assert(offsetof(AceConfig, block_size) == 16, "AceConfig.block_size");
_Static_assert(offsetof(AceConfig, write_index) == 48, "AceConfig.write_index");
_Static_assert(offsetof(AceConfig, reserved) == 51, "AceConfig.reserved");
_Static_assert(sizeof(AceLimits) == 56, "AceLimits size");
_Static_assert(offsetof(AceLimits, max_blocks) == 48, "AceLimits.max_blocks");

#define CHECK(condition)                                                              \
    do {                                                                              \
        if (!(condition)) {                                                           \
            fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #condition); \
            report_last_error();                                                      \
            return 1;                                                                 \
        }                                                                             \
    } while (0)

static void report_last_error(void) {
    size_t len = ace_last_error_len();
    if (len > 0) {
        fprintf(stderr, "last error: %.*s\n", (int)len, (const char *)ace_last_error_ptr());
    }
}

int main(void) {
    CHECK(ace_abi_version() == ACE_ABI_VERSION);
    CHECK(ace_engine_version() != NULL);
    printf("libace_ffi %s, ABI %u\n", ace_engine_version(), ace_abi_version());

    /* 200,000 little-endian counters: compressible, several blocks at the default size. */
    const size_t count = 200000;
    const size_t size = count * 4;
    uint8_t *data = malloc(size);
    CHECK(data != NULL);
    for (size_t i = 0; i < count; i++) {
        uint32_t value = (uint32_t)(i / 3);
        memcpy(data + 4 * i, &value, 4);
    }

    AceConfig config;
    ace_config_default(&config);
    CHECK(config.struct_size == sizeof(AceConfig));
    config.block_size = 64 * 1024;

    AceEngine *engine = NULL;
    CHECK(ace_engine_new(&config, NULL, &engine) == ACE_OK);
    CHECK(engine != NULL);

    AceBuffer *packed = NULL;
    AceBuffer *stats = NULL;
    CHECK(ace_compress(engine, data, size, &packed, &stats) == ACE_OK);
    CHECK(ace_buffer_len(packed) > 0 && ace_buffer_len(packed) < size);
    CHECK(strstr((const char *)ace_buffer_data(stats), "\"input_bytes\":800000") != NULL);

    AceBuffer *restored = NULL;
    CHECK(ace_decompress(engine, ace_buffer_data(packed), ace_buffer_len(packed), &restored) == ACE_OK);
    CHECK(ace_buffer_len(restored) == size);
    CHECK(memcmp(ace_buffer_data(restored), data, size) == 0);

    /* Random access: bytes [100000, 100016) without decoding the rest. */
    AceRangeReader *reader = NULL;
    CHECK(ace_range_open(engine, ace_buffer_data(packed), ace_buffer_len(packed), &reader) == ACE_OK);
    uint64_t length = 0;
    CHECK(ace_range_len(reader, &length) == ACE_OK && length == size);
    AceBuffer *slice = NULL;
    CHECK(ace_range_read(reader, 100000, 100016, &slice) == ACE_OK);
    CHECK(ace_buffer_len(slice) == 16);
    CHECK(memcmp(ace_buffer_data(slice), data + 100000, 16) == 0);

    /* A failure reports a status and a message, and leaves the output null. */
    AceBuffer *none = NULL;
    const uint8_t garbage[32] = {0x42};
    CHECK(ace_decompress(engine, garbage, sizeof garbage, &none) == ACE_ERR_NOT_ACE_FILE);
    CHECK(none == NULL);
    CHECK(ace_last_error_len() > 0);
    CHECK(ace_last_error_block() == UINT64_MAX);

    ace_buffer_free(slice);
    ace_range_free(reader);
    ace_buffer_free(restored);
    ace_buffer_free(stats);
    ace_buffer_free(packed);
    ace_engine_free(engine);
    ace_engine_free(NULL);
    free(data);
    printf("ace smoke ok\n");
    return 0;
}
