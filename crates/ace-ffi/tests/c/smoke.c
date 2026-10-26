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
_Static_assert(sizeof(AceConfig) == 112, "AceConfig size");
_Static_assert(offsetof(AceConfig, schema) == 56, "AceConfig.schema (ABI 1.1)");
_Static_assert(offsetof(AceConfig, dictionary_scope) == 72, "AceConfig.dictionary_scope");
_Static_assert(offsetof(AceConfig, max_segments) == 104, "AceConfig.max_segments");
_Static_assert(sizeof(AceColumn) == 8, "AceColumn size");
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
    CHECK(ace_abi_minor() == ACE_ABI_MINOR && ACE_ABI_MINOR >= 1);
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

    /* ABI 1.1: an external dictionary. Text records that share a vocabulary. */
    {
        enum { RECORDS = 400 };
        static uint8_t text[RECORDS * 96];
        const uint8_t *samples[RECORDS];
        size_t lengths[RECORDS];
        size_t used = 0;
        for (int i = 0; i < RECORDS; i++) {
            int n = snprintf((char *)text + used, 96,
                             "{\"sensor\":\"temperature_reading\",\"site\":\"plant-%d\",\"value\":%d}\n",
                             i % 7, 1000 + (i * 13) % 97);
            samples[i] = text + used;
            lengths[i] = (size_t)n;
            used += (size_t)n;
        }
        AceDictionary *dictionary = NULL;
        CHECK(ace_dictionary_train(samples, lengths, RECORDS, 1024, &dictionary) == ACE_OK);
        CHECK(ace_dictionary_len(dictionary) > 0 && ace_dictionary_id(dictionary) != 0);

        AceConfig external;
        ace_config_default(&external);
        external.block_size = 2048;
        external.dictionary = dictionary;
        external.dictionary_scope = ACE_DICTIONARY_EXTERNAL;
        AceEngine *writer = NULL;
        CHECK(ace_engine_new_ex(&external, NULL, NULL, 0, &writer) == ACE_OK);
        AceBuffer *with_dictionary = NULL;
        CHECK(ace_compress(writer, text, used, &with_dictionary, NULL) == ACE_OK);

        /* The id alone is not enough; an engine that was given the dictionary reads the file. */
        AceBuffer *back = NULL;
        CHECK(ace_decompress(engine, ace_buffer_data(with_dictionary), ace_buffer_len(with_dictionary), &back) == ACE_ERR_UNSUPPORTED);
        CHECK(back == NULL);
        const AceDictionary *known[1] = {dictionary};
        AceEngine *aware = NULL;
        CHECK(ace_engine_new_ex(NULL, NULL, known, 1, &aware) == ACE_OK);
        CHECK(ace_decompress(aware, ace_buffer_data(with_dictionary), ace_buffer_len(with_dictionary), &back) == ACE_OK);
        CHECK(ace_buffer_len(back) == used && memcmp(ace_buffer_data(back), text, used) == 0);

        ace_buffer_free(back);
        ace_engine_free(aware);
        ace_buffer_free(with_dictionary);
        ace_engine_free(writer);
        ace_dictionary_free(dictionary);
    }

    /* ABI 1.1: a schema describes a table of 16-byte records (u64 counter, f64 measurement). */
    {
        const AceColumn columns[2] = {{ACE_COLUMN_UNSIGNED, 8}, {ACE_COLUMN_FLOAT, 8}};
        AceSchema *schema = NULL;
        CHECK(ace_schema_new(columns, 2, &schema) == ACE_OK);
        CHECK(ace_schema_record_size(schema) == 16);
        AceSchema *rejected = NULL;
        const AceColumn bad[1] = {{ACE_COLUMN_UNSIGNED, 3}};
        CHECK(ace_schema_new(bad, 1, &rejected) == ACE_ERR_INVALID_ARGUMENT && rejected == NULL);
        ace_schema_free(schema);
        ace_schema_free(NULL);
    }

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
