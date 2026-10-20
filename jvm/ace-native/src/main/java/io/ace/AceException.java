package io.ace;

import java.util.OptionalLong;

/**
 * A failure reported by the native library. {@link #status()} says what to do about it, the
 * message says what happened.
 */
public class AceException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    private final Status status;
    private final int rawStatus;
    private final long blockId;

    /**
     * @param status the mapped status
     * @param rawStatus the numeric status as the library returned it
     * @param message the library's last-error message
     * @param blockId the block the failure concerns, or {@code -1}
     */
    AceException(Status status, int rawStatus, String message, long blockId) {
        super(message);
        this.status = status;
        this.rawStatus = rawStatus;
        this.blockId = blockId;
    }

    /** What kind of failure this is. */
    public Status status() {
        return status;
    }

    /** The numeric status of the C ABI, also for statuses newer than this SDK. */
    public int rawStatus() {
        return rawStatus;
    }

    /** The block the failure concerns ({@link Status#CHECKSUM_MISMATCH}, {@link Status#BLOCK_NOT_FOUND}). */
    public OptionalLong blockId() {
        return blockId < 0 ? OptionalLong.empty() : OptionalLong.of(blockId);
    }
}
