package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.List;
import org.junit.jupiter.api.Test;

class EngineTest {
    private static final List<byte[]> INPUTS = List.of(
            new byte[0],
            new byte[] {42},
            TestData.counters(100_000),
            TestData.text(300_000),
            TestData.random(70_000, 7),
            TestData.doubles(40_000));

    @Test
    void roundTripsEveryKindOfInputUnderEveryProfile() {
        for (Profile profile : Profile.values()) {
            try (Engine engine = Engine.create(Config.builder().profile(profile).blockSize(64 * 1024).build())) {
                for (byte[] input : INPUTS) {
                    byte[] packed = engine.compress(input);
                    assertArrayEquals(input, engine.decompress(packed), profile + " " + input.length);
                }
            }
        }
    }

    @Test
    void statsDescribeTheRun() {
        byte[] input = TestData.counters(200_000);
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            Compressed result = engine.compressWithStats(input);
            Stats stats = result.stats();
            assertEquals(input.length, stats.inputBytes());
            assertEquals(result.bytes().length, stats.outputBytes());
            assertEquals((input.length + 32 * 1024 - 1) / (32 * 1024), stats.blocks());
            assertTrue(stats.ratio() > 2.0, "ratio " + stats.ratio());
            assertFalse(stats.planDistribution().isEmpty());
            assertEquals(stats.blocks(), stats.planDistribution().values().stream().mapToLong(Long::longValue).sum());
        }
    }

    @Test
    void outputDoesNotDependOnTheThreadCount() {
        byte[] input = TestData.counters(300_000);
        byte[] reference = null;
        for (int threads : new int[] {1, 2, 5}) {
            try (Engine engine = Engine.create(Config.builder().blockSize(32 * 1024).threads(threads).build())) {
                byte[] packed = engine.compress(input);
                if (reference == null) {
                    reference = packed;
                }
                assertArrayEquals(reference, packed, threads + " threads");
            }
        }
    }

    @Test
    void profilesTradeEffortForSize() {
        byte[] input = TestData.doubles(60_000);
        try (Engine fast = Engine.create(Config.builder().profile(Profile.FAST).build());
                Engine dense = Engine.create(Config.builder().profile(Profile.DENSE).build())) {
            assertTrue(dense.compress(input).length <= fast.compress(input).length);
        }
    }

    @Test
    void explainListsEveryBlock() {
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            String json = engine.explainJson(TestData.counters(40_000));
            assertTrue(json.startsWith("[{\"block_id\":0,"), json);
            assertTrue(json.contains("\"candidates\":["));
        }
    }

    @Test
    void decodingFailuresCarryTheirStatus() {
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            AceException notAce = assertThrows(AceException.class, () -> engine.decompress(new byte[64]));
            assertEquals(Status.NOT_ACE_FILE, notAce.status());
            assertFalse(notAce.getMessage().isEmpty());
            assertTrue(notAce.blockId().isEmpty());

            byte[] packed = engine.compress(TestData.counters(100_000));
            byte[] damaged = packed.clone();
            damaged[damaged.length / 2] ^= (byte) 0xFF;
            AceException corrupt = assertThrows(AceException.class, () -> engine.decompress(damaged));
            assertTrue(corrupt.status() == Status.CHECKSUM_MISMATCH || corrupt.status() == Status.CORRUPT,
                    "status " + corrupt.status());

            byte[] truncated = java.util.Arrays.copyOf(packed, packed.length / 2);
            assertThrows(AceException.class, () -> engine.decompress(truncated));
        }
    }

    @Test
    void decodeLimitsApply() {
        byte[] input = TestData.counters(100_000);
        byte[] packed;
        try (Engine engine = Engine.create()) {
            packed = engine.compress(input);
        }
        DecodeLimits strict = DecodeLimits.builder().maxOutputSize(1000).build();
        try (Engine engine = Engine.create(Config.defaults(), strict)) {
            AceException e = assertThrows(AceException.class, () -> engine.decompress(packed));
            assertEquals(Status.LIMIT_EXCEEDED, e.status());
            assertEquals(1000, engine.limits().maxOutputSize());
        }
    }

    @Test
    void invalidSettingsAreRejected() {
        assertThrows(IllegalArgumentException.class, () -> Config.builder().blockSize(-1));
        assertThrows(IllegalArgumentException.class, () -> DecodeLimits.builder().maxBlocks(-5));
        assertThrows(NullPointerException.class, () -> Config.builder().profile(null));
        AceException e = assertThrows(AceException.class,
                () -> Engine.create(Config.builder().blockSize(1L << 40).build()));
        assertEquals(Status.INVALID_ARGUMENT, e.status());
    }

    @Test
    void zeroCopyRoundTrip() {
        byte[] input = TestData.counters(150_000);
        try (Engine engine = Engine.create(TestData.smallBlocks()); Arena arena = Arena.ofConfined()) {
            MemorySegment source = arena.allocate(input.length);
            source.copyFrom(MemorySegment.ofArray(input));
            try (NativeBuffer packed = engine.compress(source)) {
                assertTrue(packed.size() < input.length / 2);
                assertArrayEquals(engine.compress(input), packed.toByteArray());
                try (NativeBuffer restored = engine.decompress(packed.segment())) {
                    assertEquals(input.length, restored.size());
                    assertArrayEquals(input, restored.toByteArray());
                    assertEquals(input[1234], restored.segment().get(ValueLayout.JAVA_BYTE, 1234));
                }
            }
        }
    }

    @Test
    void decompressIntoFillsACallerSegment() {
        byte[] input = TestData.text(100_000);
        try (Engine engine = Engine.create(); Arena arena = Arena.ofConfined()) {
            byte[] packed = engine.compress(input);
            MemorySegment file = arena.allocate(packed.length);
            file.copyFrom(MemorySegment.ofArray(packed));

            MemorySegment small = arena.allocate(10);
            BufferTooSmallException tooSmall = assertThrows(BufferTooSmallException.class,
                    () -> engine.decompressInto(file, small));
            assertEquals(input.length, tooSmall.requiredSize());
            assertEquals(Status.BUFFER_TOO_SMALL, tooSmall.status());

            MemorySegment exact = arena.allocate(input.length);
            assertEquals(input.length, engine.decompressInto(file, exact));
            assertArrayEquals(input, exact.toArray(ValueLayout.JAVA_BYTE));
        }
    }

    @Test
    void streamingEqualsOneShot() throws IOException {
        byte[] input = TestData.counters(250_000);
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            ByteArrayOutputStream packed = new ByteArrayOutputStream();
            Stats stats = engine.compressStream(input.length, new ByteArrayInputStream(input), packed);
            assertArrayEquals(engine.compress(input), packed.toByteArray());
            assertEquals(input.length, stats.inputBytes());
            assertEquals(packed.size(), stats.outputBytes());

            ByteArrayOutputStream restored = new ByteArrayOutputStream();
            engine.decompressStream(new ByteArrayInputStream(packed.toByteArray()), restored);
            assertArrayEquals(input, restored.toByteArray());
        }
    }

    @Test
    void streamFailuresReachTheCaller() {
        byte[] input = TestData.counters(100_000);
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            InputStream broken = new InputStream() {
                @Override
                public int read() throws IOException {
                    throw new IOException("disk on fire");
                }

                @Override
                public int read(byte[] buffer, int offset, int length) throws IOException {
                    throw new IOException("disk on fire");
                }
            };
            IOException read = assertThrows(IOException.class,
                    () -> engine.compressStream(input.length, broken, new ByteArrayOutputStream()));
            assertEquals("disk on fire", read.getMessage());

            byte[] packed = engine.compress(input);
            OutputStream full = new OutputStream() {
                @Override
                public void write(int b) throws IOException {
                    throw new IOException("disk full");
                }

                @Override
                public void write(byte[] buffer, int offset, int length) throws IOException {
                    throw new IOException("disk full");
                }
            };
            IOException write = assertThrows(IOException.class,
                    () -> engine.decompressStream(new ByteArrayInputStream(packed), full));
            assertEquals("disk full", write.getMessage());

            // The wrong declared size is reported by the library.
            assertThrows(AceException.class,
                    () -> engine.compressStream(input.length + 5, new ByteArrayInputStream(input),
                            new ByteArrayOutputStream()));
        }
    }

    @Test
    void differentConfigsProduceDifferentButValidFiles() {
        byte[] input = TestData.text(200_000);
        try (Engine small = Engine.create(Config.builder().blockSize(16 * 1024).build());
                Engine large = Engine.create(Config.builder().blockSize(128 * 1024).build())) {
            byte[] a = small.compress(input);
            byte[] b = large.compress(input);
            assertNotEquals(a.length, b.length);
            assertArrayEquals(input, large.decompress(a));
            assertArrayEquals(input, small.decompress(b));
        }
    }
}
