package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import org.junit.jupiter.api.Test;

class RangeReaderTest {
    private static final int BLOCK = 32 * 1024;

    @Test
    void readsRangesAndBlocks() {
        byte[] input = TestData.counters(200_000);
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            byte[] packed = engine.compress(input);
            try (RangeReader reader = engine.openRange(packed)) {
                assertEquals(input.length, reader.length());
                assertEquals((input.length + BLOCK - 1) / BLOCK, reader.blockCount());
                long[][] ranges = {{0, 10}, {BLOCK - 5, BLOCK + 5}, {1000, 300_000}, {input.length - 1, input.length}, {7, 7}};
                for (long[] range : ranges) {
                    assertArrayEquals(Arrays.copyOfRange(input, (int) range[0], (int) range[1]),
                            reader.read(range[0], range[1]), range[0] + ".." + range[1]);
                }
                assertArrayEquals(Arrays.copyOfRange(input, BLOCK, 2 * BLOCK), reader.readBlock(1));
            }
        }
    }

    @Test
    void reportsTheWorkOfTheLastRead() {
        byte[] input = TestData.counters(200_000);
        try (Engine engine = Engine.create(TestData.smallBlocks()); RangeReader reader = engine.openRange(engine.compress(input))) {
            reader.read(100, 100 + BLOCK + 10);
            RangeStats stats = reader.lastStats();
            assertEquals(BLOCK + 10, stats.logicalBytes());
            assertTrue(stats.blocks() >= 2);
            assertTrue(stats.physicalBytes() > 0);
            assertTrue(stats.readAmplification() > 0.0);
        }
    }

    @Test
    void rejectsBadRanges() {
        try (Engine engine = Engine.create(TestData.smallBlocks());
                RangeReader reader = engine.openRange(engine.compress(TestData.text(100_000)))) {
            assertEquals(Status.INVALID_ARGUMENT, assertThrows(AceException.class, () -> reader.read(10, 5)).status());
            assertEquals(Status.INVALID_ARGUMENT,
                    assertThrows(AceException.class, () -> reader.read(0, reader.length() + 1)).status());
            assertEquals(Status.BLOCK_NOT_FOUND,
                    assertThrows(AceException.class, () -> reader.readBlock(reader.blockCount())).status());
        }
    }

    @Test
    void opensFromANativeSegmentAndFromAFile() throws IOException {
        byte[] input = TestData.counters(120_000);
        try (Engine engine = Engine.create(TestData.smallBlocks()); Arena arena = Arena.ofConfined()) {
            byte[] packed = engine.compress(input);
            MemorySegment segment = arena.allocate(packed.length);
            segment.copyFrom(MemorySegment.ofArray(packed));
            try (RangeReader reader = engine.openRange(segment)) {
                assertArrayEquals(Arrays.copyOfRange(input, 500, 900), reader.read(500, 900));
            }

            Path file = Files.createTempFile("ace-range", ".ace");
            try {
                Files.write(file, packed);
                try (RangeReader reader = engine.openRange(file)) {
                    assertArrayEquals(Arrays.copyOfRange(input, 40_000, 80_000), reader.read(40_000, 80_000));
                }
                assertEquals(Status.IO, assertThrows(AceException.class,
                        () -> engine.openRange(file.resolveSibling("does-not-exist.ace"))).status());
            } finally {
                Files.deleteIfExists(file);
            }
        }
    }

    @Test
    void aFileWithoutAnIndexCannotBeRangeRead() {
        try (Engine plain = Engine.create(Config.builder().writeIndex(false).build());
                Engine engine = Engine.create()) {
            byte[] packed = plain.compress(TestData.text(50_000));
            assertThrows(AceException.class, () -> engine.openRange(packed));
            assertArrayEquals(TestData.text(50_000), engine.decompress(packed));
        }
    }
}
