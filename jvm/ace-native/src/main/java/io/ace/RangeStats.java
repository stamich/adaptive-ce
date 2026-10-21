package io.ace;

/**
 * The work one range read did.
 *
 * @param logicalBytes bytes of the original file that were asked for
 * @param physicalBytes bytes of the compressed file that had to be read for them
 * @param blocks independent blocks that were decoded
 */
public record RangeStats(long logicalBytes, long physicalBytes, long blocks) {
    /** Compressed bytes read per original byte returned; 0 for an empty range. */
    public double readAmplification() {
        return logicalBytes == 0 ? 0.0 : (double) physicalBytes / (double) logicalBytes;
    }
}
