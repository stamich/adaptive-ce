package io.ace.demo;

import io.ace.Compressed;
import io.ace.Config;
import io.ace.Engine;
import io.ace.Profile;
import io.ace.RangeReader;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;

/** ACE from Java: compress with statistics, verify, and read a range without decoding the file. */
public final class JavaDemo {
    private JavaDemo() {}

    /** Little-endian u64 timestamps with small jitter: the numeric pipelines compress them well. */
    static byte[] timestamps(int count) {
        ByteBuffer buffer = ByteBuffer.allocate(count * 8).order(ByteOrder.LITTLE_ENDIAN);
        long time = 1_700_000_000_000_000_000L;
        for (int i = 0; i < count; i++) {
            time += 1_000_000L + (i * 7919L) % 200;
            buffer.putLong(time);
        }
        return buffer.array();
    }

    /** Runs the demo and returns what it prints, so the test can check it. */
    public static String run() {
        StringBuilder out = new StringBuilder();
        byte[] input = timestamps(500_000);
        Config config = Config.builder().profile(Profile.BALANCED).blockSize(256 * 1024).build();
        try (Engine engine = Engine.create(config)) {
            Compressed packed = engine.compressWithStats(input);
            out.append("input   %,d bytes%n".formatted(input.length));
            out.append("output  %,d bytes (ratio %.2f, %d blocks)%n"
                    .formatted(packed.bytes().length, packed.stats().ratio(), packed.stats().blocks()));
            out.append("plans   %s%n".formatted(packed.stats().planDistribution()));

            byte[] restored = engine.decompress(packed.bytes());
            if (!Arrays.equals(input, restored)) {
                throw new IllegalStateException("round trip mismatch");
            }
            out.append("round trip verified%n".formatted());

            try (RangeReader reader = engine.openRange(packed.bytes())) {
                byte[] slice = reader.read(1_000_000, 1_000_064);
                if (!Arrays.equals(slice, Arrays.copyOfRange(input, 1_000_000, 1_000_064))) {
                    throw new IllegalStateException("range mismatch");
                }
                out.append("range   64 bytes at 1,000,000 read by decoding %d block(s), %d compressed bytes%n"
                        .formatted(reader.lastStats().blocks(), reader.lastStats().physicalBytes()));
            }
        }
        return out.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
