package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/** Format 1.6 through the SDK: dictionaries, schemas and segmentation (docs/DICTIONARIES.md). */
class StorageTest {
    /** One small JSON record; records share a vocabulary, which is what a dictionary captures. */
    private static byte[] record(int i) {
        return ("{\"sensor\":\"temperature_reading\",\"unit\":\"celsius\",\"site\":\"plant-" + (i % 7)
                        + "\",\"value\":" + (1000 + i * 13 % 97) + "}\n")
                .getBytes(StandardCharsets.UTF_8);
    }

    private static List<byte[]> samples(int count) {
        List<byte[]> list = new ArrayList<>();
        for (int i = 0; i < count; i++) {
            list.add(record(i));
        }
        return list;
    }

    private static byte[] records(int count) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (int i = 0; i < count; i++) {
            out.writeBytes(record(i));
        }
        return out.toByteArray();
    }

    /** Records of a u64 counter, an f64 measurement and a u64 flag word. */
    private static byte[] table(int rows) {
        ByteBuffer buffer = ByteBuffer.allocate(rows * 24).order(ByteOrder.LITTLE_ENDIAN);
        for (long i = 0; i < rows; i++) {
            buffer.putLong(1_700_000_000L + i * 5).putDouble(100 + (i % 50) * 0.25).putLong(i % 3);
        }
        return buffer.array();
    }

    private static Config blocks4k(Dictionary dictionary, boolean external) {
        Config.Builder builder = Config.builder().blockSize(4096);
        return (external ? builder.externalDictionary(dictionary) : builder.dictionary(dictionary)).build();
    }

    @Test
    void trainingIsDeterministicAndKeepsTheContent() {
        Dictionary first = Dictionary.train(samples(300), 2048);
        Dictionary second = Dictionary.train(samples(300), 2048);
        assertEquals(first, second);
        assertTrue(first.size() > 0 && first.size() <= 2048);
        assertNotEquals(0L, first.id());
        assertEquals(first, Dictionary.of(first.bytes()), "the id is derived from the bytes");
        assertEquals(42L, Dictionary.of(42, new byte[] {1, 2, 3}).id());
    }

    @Test
    void dictionaryBytesAreCopied() {
        byte[] source = "temperature_reading".getBytes(StandardCharsets.UTF_8);
        Dictionary dictionary = Dictionary.of(source);
        source[0] = 0;
        dictionary.bytes()[1] = 0;
        assertEquals("temperature_reading", new String(dictionary.bytes(), StandardCharsets.UTF_8));
    }

    @Test
    void invalidDictionariesAreRejected() {
        assertEquals(Status.INVALID_ARGUMENT,
                assertThrows(AceException.class, () -> Dictionary.of(new byte[0])).status());
        assertEquals(Status.INVALID_ARGUMENT,
                assertThrows(AceException.class, () -> Dictionary.of(new byte[Dictionary.MAX_SIZE + 1])).status());
        assertThrows(IllegalArgumentException.class, () -> Dictionary.of(0, new byte[] {1}));
        assertEquals(Status.INVALID_ARGUMENT,
                assertThrows(AceException.class, () -> Dictionary.train(samples(1), 1024)).status());
    }

    @Test
    void embeddedDictionaryNeedsNothingOnTheReadingSide() {
        Dictionary dictionary = Dictionary.train(samples(300), 2048);
        byte[] input = records(600);
        try (Engine writer = Engine.create(blocks4k(dictionary, false));
                Engine plain = Engine.create()) {
            Compressed packed = writer.compressWithStats(input);
            assertEquals(6, packed.stats().formatMinor());
            assertTrue(packed.stats().dictionaryBlocks() > 0);
            assertArrayEquals(input, plain.decompress(packed.bytes()));
        }
    }

    @Test
    void externalDictionaryNeedsTheReaderToSupplyIt() {
        Dictionary dictionary = Dictionary.train(samples(300), 2048);
        byte[] input = records(600);
        try (Engine writer = Engine.create(blocks4k(dictionary, true));
                Engine plain = Engine.create();
                Engine reader = Engine.create(Config.defaults(), DecodeLimits.defaults(), List.of(dictionary))) {
            byte[] packed = writer.compress(input);
            AceException missing = assertThrows(AceException.class, () -> plain.decompress(packed));
            assertEquals(Status.UNSUPPORTED, missing.status());
            assertTrue(missing.getMessage().contains(Long.toUnsignedString(dictionary.id())), missing.getMessage());
            assertArrayEquals(input, reader.decompress(packed));
            try (RangeReader range = reader.openRange(packed)) {
                assertArrayEquals(java.util.Arrays.copyOfRange(input, 5, 25), range.read(5, 25));
            }
        }
    }

    @Test
    void aDictionaryOnlyHelpsWhenItPays() {
        Dictionary dictionary = Dictionary.train(samples(300), 2048);
        byte[] input = records(600);
        try (Engine with = Engine.create(blocks4k(dictionary, true));
                Engine without = Engine.create(Config.builder().blockSize(4096).build())) {
            assertTrue(with.compress(input).length < without.compress(input).length);
        }
    }

    @Test
    void configExposesTheStorageOptions() {
        Dictionary dictionary = Dictionary.of(new byte[] {1, 2, 3});
        Schema schema = Schema.of(Column.unsigned(8));
        Segmentation segmentation = Segmentation.defaults().withMinSegmentSize(1 << 17);
        Config config = Config.builder().schema(schema).externalDictionary(dictionary).segmentation(segmentation).build();
        assertEquals(schema, config.schema());
        assertEquals(dictionary, config.dictionary());
        assertTrue(config.dictionaryIsExternal());
        assertEquals(1 << 17, config.segmentation().minSegmentSize());
        assertFalse(Config.builder().dictionary(dictionary).build().dictionaryIsExternal());
        assertNull(Config.defaults().schema());
        assertNull(Config.defaults().dictionary());
        assertNull(Config.defaults().segmentation());
        assertFalse(Config.defaults().dictionaryIsExternal());
    }

    @Test
    void schemaStoresATableColumnByColumn() {
        Schema schema = Schema.of(Column.unsigned(8), Column.floating(8), Column.unsigned(8));
        assertEquals(24, schema.recordSize());
        byte[] input = table(20_000);
        try (Engine columnar = Engine.create(Config.builder().schema(schema).build());
                Engine rows = Engine.create()) {
            Compressed packed = columnar.compressWithStats(input);
            assertTrue(packed.stats().columnarBlocks() > 0);
            assertTrue(packed.bytes().length < rows.compress(input).length);
            assertArrayEquals(input, rows.decompress(packed.bytes()), "readers need no schema");
        }
    }

    @Test
    void invalidColumnsAreRejected() {
        assertThrows(IllegalArgumentException.class, () -> Column.unsigned(3));
        assertThrows(IllegalArgumentException.class, () -> Column.floating(2));
        assertThrows(IllegalArgumentException.class, () -> Column.bytes(0));
        assertThrows(IllegalArgumentException.class, () -> Schema.of());
    }

    @Test
    void segmentationRoundTripsAndNeverCostsSpace() {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.writeBytes(TestData.counters(400_000));
        out.writeBytes(TestData.random(1_500_000, 3));
        byte[] input = out.toByteArray();
        Segmentation segmentation = Segmentation.defaults().withMinSegmentSize(128 * 1024);
        try (Engine segmented = Engine.create(Config.builder().blockSize(4 << 20).segmentation(segmentation).build());
                Engine whole = Engine.create(Config.builder().blockSize(4 << 20).build())) {
            Compressed packed = segmented.compressWithStats(input);
            assertTrue(packed.bytes().length <= whole.compress(input).length);
            assertArrayEquals(input, whole.decompress(packed.bytes()));
        }
    }

    @Test
    void negativeSegmentationValuesAreRejected() {
        assertThrows(IllegalArgumentException.class, () -> Segmentation.defaults().withMaxSegments(-1));
    }

    @Test
    void statsParseTheFormat16CountersAndTheOldJson() {
        Stats now = Stats.fromJson("{\"input_bytes\":10,\"output_bytes\":5,\"format_minor\":6,\"blocks\":2,"
                + "\"plan_distribution\":{},\"segmented_blocks\":1,\"columnar_blocks\":2,\"dictionary_blocks\":3}");
        assertEquals(1, now.segmentedBlocks());
        assertEquals(2, now.columnarBlocks());
        assertEquals(3, now.dictionaryBlocks());
        Stats old = Stats.fromJson("{\"input_bytes\":10,\"output_bytes\":5,\"format_minor\":5,\"blocks\":2,"
                + "\"plan_distribution\":{}}");
        assertEquals(0, old.segmentedBlocks() + old.columnarBlocks() + old.dictionaryBlocks());
    }

    @Test
    void theNativeLibraryReportsAbiMinorOne() {
        assertTrue(NativeLibrary.functions().aceAbiMinor() >= 1);
    }
}
