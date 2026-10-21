package io.ace;

/** A caller-provided output segment is too small; {@link #requiredSize()} says how large it must be. */
public final class BufferTooSmallException extends AceException {
    private static final long serialVersionUID = 1L;

    private final long requiredSize;

    BufferTooSmallException(int rawStatus, String message, long requiredSize) {
        super(Status.BUFFER_TOO_SMALL, rawStatus, message, -1);
        this.requiredSize = requiredSize;
    }

    /** The number of bytes the decoded output needs. */
    public long requiredSize() {
        return requiredSize;
    }
}
