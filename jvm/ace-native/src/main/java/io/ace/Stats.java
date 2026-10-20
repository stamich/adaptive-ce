package io.ace;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Summary of one compression run (docs/FFI.md, "Stats JSON").
 *
 * @param inputBytes bytes read
 * @param outputBytes bytes of the ACE file written
 * @param formatMinor minor version of the file format written ({@code 1.<minor>})
 * @param blocks number of blocks
 * @param rawBlocks blocks stored as is
 * @param rleBlocks blocks coded as run-length
 * @param lzBlocks blocks coded with the LZ codec
 * @param numericBlocks blocks coded with the integer codecs
 * @param timeSeriesBlocks blocks coded with the floating-point and time-series codec
 * @param ratio {@code inputBytes / outputBytes}; 1.0 when nothing was written
 * @param planDistribution how many blocks use each pipeline, keyed by its label
 */
public record Stats(
        long inputBytes,
        long outputBytes,
        int formatMinor,
        long blocks,
        long rawBlocks,
        long rleBlocks,
        long lzBlocks,
        long numericBlocks,
        long timeSeriesBlocks,
        double ratio,
        Map<String, Long> planDistribution) {

    /** Makes the distribution unmodifiable. */
    public Stats {
        planDistribution = Collections.unmodifiableMap(new LinkedHashMap<>(planDistribution));
    }

    /** Parses the JSON that {@code ace_compress} returns. */
    public static Stats fromJson(String json) {
        Map<String, Object> root = Json.parseObject(json);
        Map<String, Long> plans = new LinkedHashMap<>();
        Object distribution = root.get("plan_distribution");
        if (distribution instanceof Map<?, ?> map) {
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                plans.put(String.valueOf(entry.getKey()), ((Number) entry.getValue()).longValue());
            }
        }
        return new Stats(
                number(root, "input_bytes"),
                number(root, "output_bytes"),
                (int) number(root, "format_minor"),
                number(root, "blocks"),
                number(root, "raw_blocks"),
                number(root, "rle_blocks"),
                number(root, "lz_blocks"),
                number(root, "numeric_blocks"),
                number(root, "time_series_blocks"),
                ((Number) root.getOrDefault("ratio", 1.0)).doubleValue(),
                plans);
    }

    private static long number(Map<String, Object> root, String key) {
        Object value = root.get(key);
        return value instanceof Number number ? number.longValue() : 0L;
    }
}
