package io.ace;

/**
 * Lets ACE split a block into segments with different codecs, for data that changes character
 * inside one block (a text header, then counters, then random bytes). A block is stored
 * segmented only when that is smaller than storing it whole, so enabling segmentation never
 * costs space. Readers need no setting; random access reads only the segments it needs.
 *
 * <p>A value of 0 keeps the library default.
 *
 * <pre>{@code
 * Config config = Config.builder()
 *     .blockSize(4 << 20)
 *     .segmentation(Segmentation.defaults().withMinSegmentSize(128 * 1024))
 *     .build();
 * }</pre>
 *
 * @param windowSize granularity of the classification in bytes (default 16 KiB)
 * @param minSegmentSize smallest segment in bytes (default 64 KiB)
 * @param maxSegmentSize largest segment in bytes (default 1 MiB)
 * @param maxSegments most segments in one block (default 1 024)
 */
public record Segmentation(long windowSize, long minSegmentSize, long maxSegmentSize, long maxSegments) {
    private static final Segmentation DEFAULTS = new Segmentation(0, 0, 0, 0);

    /** Rejects negative values. */
    public Segmentation {
        Config.nonNegative(windowSize, "windowSize");
        Config.nonNegative(minSegmentSize, "minSegmentSize");
        Config.nonNegative(maxSegmentSize, "maxSegmentSize");
        Config.nonNegative(maxSegments, "maxSegments");
    }

    /** Segmentation with the library defaults. */
    public static Segmentation defaults() {
        return DEFAULTS;
    }

    /** A copy with another classification window in bytes. */
    public Segmentation withWindowSize(long bytes) {
        return new Segmentation(bytes, minSegmentSize, maxSegmentSize, maxSegments);
    }

    /** A copy with another smallest segment in bytes. */
    public Segmentation withMinSegmentSize(long bytes) {
        return new Segmentation(windowSize, bytes, maxSegmentSize, maxSegments);
    }

    /** A copy with another largest segment in bytes. */
    public Segmentation withMaxSegmentSize(long bytes) {
        return new Segmentation(windowSize, minSegmentSize, bytes, maxSegments);
    }

    /** A copy with another limit of segments per block. */
    public Segmentation withMaxSegments(long count) {
        return new Segmentation(windowSize, minSegmentSize, maxSegmentSize, count);
    }
}
