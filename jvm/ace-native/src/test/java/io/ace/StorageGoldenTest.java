package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

/**
 * The Java SDK writes exactly the bytes of the committed 0.8.0 storage golden file
 * ({@code examples/golden/0.8.0/GOLDEN.json}): columnar blocks from a schema, segments, and
 * embedded and external dictionaries trained through {@link Dictionary#train}, for every profile.
 * The record inputs come from {@code ace-corpus}; the others are generated here, and every input
 * is checked against its digest first.
 */
class StorageGoldenTest {
    private static String sha256(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    @SuppressWarnings("unchecked")
    private static List<Map<String, Object>> cases() throws IOException {
        Path root = Path.of(System.getProperty("ace.repo.root"));
        Map<String, Object> document = Json.parseObject(
                Files.readString(root.resolve("examples/golden/0.8.0/GOLDEN.json")));
        assertEquals("ace-golden-storage-1", document.get("schema"));
        return (List<Map<String, Object>>) document.get("cases");
    }

    private static byte[] jsonRecord(int i) {
        return ("{\"sensor\":\"temperature_reading\",\"unit\":\"celsius\",\"site\":\"plant-" + (i % 7)
                        + "\",\"value\":" + (1000 + i * 13 % 97) + "}\n")
                .getBytes(StandardCharsets.UTF_8);
    }

    private static byte[] input(String generator) throws Exception {
        if (generator.startsWith("records-")) {
            int colon = generator.indexOf(':');
            return corpus(generator.substring(0, colon), generator.substring(colon + 1));
        }
        if (generator.startsWith("json-records:")) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            for (int i = 0; i < Integer.parseInt(generator.substring("json-records:".length())); i++) {
                out.writeBytes(jsonRecord(i));
            }
            return out.toByteArray();
        }
        assertEquals("mixed-regimes", generator);
        ByteBuffer buffer = ByteBuffer.allocate(400_000 * 4 + 1_500_000).order(ByteOrder.LITTLE_ENDIAN);
        for (int i = 0; i < 400_000; i++) {
            buffer.putInt(i / 2);
        }
        for (long i = 0; i < 1_500_000; i++) {
            buffer.put((byte) ((i * 2_654_435_761L) >>> 13));
        }
        return buffer.array();
    }

    private static byte[] corpus(String workload, String bytes) throws Exception {
        String binary = System.getProperty("ace.corpus.bin");
        assertTrue(binary != null && Files.isExecutable(Path.of(binary)),
                "ace.corpus.bin must name the ace-corpus binary (cargo build --release -p ace-corpus), got " + binary);
        Path file = Files.createTempFile("ace-golden-", ".bin");
        try {
            Process process = new ProcessBuilder(binary, workload, bytes, file.toString())
                    .redirectErrorStream(true)
                    .start();
            String output = new String(process.getInputStream().readAllBytes());
            assertEquals(0, process.waitFor(), output);
            return Files.readAllBytes(file);
        } finally {
            Files.deleteIfExists(file);
        }
    }

    private static Schema schema(String layout) {
        List<Column> columns = new ArrayList<>();
        for (String column : layout.split(",")) {
            int width = Integer.parseInt(column.substring(1));
            columns.add(switch (column.charAt(0)) {
                case 'u' -> Column.unsigned(width);
                case 's' -> Column.signed(width);
                case 'f' -> Column.floating(width);
                case 'b' -> Column.bytes(width);
                default -> throw new IllegalArgumentException(column);
            });
        }
        return new Schema(columns);
    }

    private static Profile profile(String name) {
        return switch (name) {
            case "fast" -> Profile.FAST;
            case "balanced" -> Profile.BALANCED;
            case "dense" -> Profile.DENSE;
            default -> throw new IllegalArgumentException(name);
        };
    }

    @Test
    void reproducesTheStorageGoldenFileForEveryCaseAndProfile() throws Exception {
        List<Map<String, Object>> all = cases();
        assertEquals(6, all.size());
        for (Map<String, Object> entry : all) {
            String name = (String) entry.get("case");
            byte[] data = input((String) entry.get("generator"));
            assertEquals(entry.get("input_sha256"), sha256(data), name + ": generator");

            String kind = (String) entry.get("dictionary");
            Dictionary dictionary = null;
            if (!kind.equals("none")) {
                List<byte[]> samples = new ArrayList<>();
                for (int i = 0; i < 300; i++) {
                    samples.add(jsonRecord(i));
                }
                dictionary = Dictionary.train(samples, 2048);
                assertEquals(entry.get("dictionary_id"), String.format("%016x", dictionary.id()), name);
                assertEquals(entry.get("dictionary_sha256"), sha256(dictionary.bytes()), name + ": trained dictionary");
            }
            for (String profile : List.of("fast", "balanced", "dense")) {
                Config.Builder builder = Config.builder()
                        .profile(profile(profile))
                        .blockSize(((Number) entry.get("block_size")).longValue());
                String layout = (String) entry.get("schema");
                if (!layout.isEmpty()) {
                    builder.schema(schema(layout));
                }
                long segmentationMin = ((Number) entry.get("segmentation_min")).longValue();
                if (segmentationMin > 0) {
                    builder.segmentation(Segmentation.defaults().withMinSegmentSize(segmentationMin));
                }
                if (kind.equals("embedded")) {
                    builder.dictionary(dictionary);
                } else if (kind.equals("external")) {
                    builder.externalDictionary(dictionary);
                }
                try (Engine engine = Engine.create(builder.build())) {
                    byte[] file = engine.compress(data);
                    assertEquals(((Number) entry.get(profile + "_bytes")).intValue(), file.length, name + " " + profile);
                    assertEquals(entry.get(profile + "_sha256"), sha256(file), name + " " + profile + ": bytes");
                    assertArrayEquals(data, engine.decompress(file), name + " " + profile + ": round trip");
                }
            }
        }
    }
}
