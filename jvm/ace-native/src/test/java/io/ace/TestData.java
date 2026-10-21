package io.ace;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Random;

/** Deterministic inputs for the tests. */
final class TestData {
    private TestData() {}

    /** Little-endian u32 counters that repeat each value three times: very compressible. */
    static byte[] counters(int count) {
        ByteBuffer buffer = ByteBuffer.allocate(count * 4).order(ByteOrder.LITTLE_ENDIAN);
        for (int i = 0; i < count; i++) {
            buffer.putInt(i / 3);
        }
        return buffer.array();
    }

    /** Text-like bytes with repetition. */
    static byte[] text(int length) {
        byte[] words = "adaptive compression engine lossless deterministic random access ".getBytes();
        byte[] out = new byte[length];
        for (int i = 0; i < length; i++) {
            out[i] = words[(i * 7 + i / words.length) % words.length];
        }
        return out;
    }

    /** Incompressible bytes. */
    static byte[] random(int length, long seed) {
        byte[] out = new byte[length];
        new Random(seed).nextBytes(out);
        return out;
    }

    /** Doubles of a slow sine: exercises the floating-point pipelines. */
    static byte[] doubles(int count) {
        ByteBuffer buffer = ByteBuffer.allocate(count * 8).order(ByteOrder.LITTLE_ENDIAN);
        for (int i = 0; i < count; i++) {
            buffer.putDouble(Math.round(Math.sin(i / 50.0) * 10000.0) / 100.0);
        }
        return buffer.array();
    }

    /** A config with small blocks, so a few hundred KiB make several blocks. */
    static Config smallBlocks() {
        return Config.builder().blockSize(32 * 1024).build();
    }
}
