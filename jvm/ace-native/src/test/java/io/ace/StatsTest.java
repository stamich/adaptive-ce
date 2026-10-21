package io.ace;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class StatsTest {
    @Test
    void parsesTheDocumentedJson() {
        Stats stats = Stats.fromJson("{\"input_bytes\":1000,\"output_bytes\":250,\"format_minor\":5,\"blocks\":2,"
                + "\"raw_blocks\":0,\"rle_blocks\":0,\"lz_blocks\":1,\"numeric_blocks\":1,"
                + "\"time_series_blocks\":0,\"ratio\":4,\"plan_distribution\":{\"lz+huffman\":1,\"na\\\"me\":1}}");
        assertEquals(1000, stats.inputBytes());
        assertEquals(250, stats.outputBytes());
        assertEquals(5, stats.formatMinor());
        assertEquals(2, stats.blocks());
        assertEquals(1, stats.lzBlocks());
        assertEquals(1, stats.numericBlocks());
        assertEquals(4.0, stats.ratio(), 1e-12);
        assertEquals(Map.of("lz+huffman", 1L, "na\"me", 1L), stats.planDistribution());
        assertThrows(UnsupportedOperationException.class, () -> stats.planDistribution().put("x", 1L));
    }

    @Test
    void unknownMembersAreIgnoredAndMissingOnesAreZero() {
        Stats stats = Stats.fromJson("{\"input_bytes\":7,\"future_member\":[1,2,{\"a\":null}]}");
        assertEquals(7, stats.inputBytes());
        assertEquals(0, stats.blocks());
        assertEquals(1.0, stats.ratio(), 1e-12);
    }

    @Test
    void jsonReaderHandlesTheBasics() {
        Object value = Json.parse(" {\"a\": [1, 2.5, \"x\\n\\u0041\", true, false, null], \"b\": {}} ");
        Map<?, ?> map = (Map<?, ?>) value;
        assertEquals(java.util.Arrays.asList(1L, 2.5, "x\nA", true, false, null), map.get("a"));
        assertEquals(Map.of(), map.get("b"));
        assertThrows(IllegalArgumentException.class, () -> Json.parse("{\"a\":}"));
        assertThrows(IllegalArgumentException.class, () -> Json.parse("{} extra"));
        assertThrows(IllegalArgumentException.class, () -> Json.parse("\"open"));
    }
}
