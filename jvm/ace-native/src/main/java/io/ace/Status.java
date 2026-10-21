package io.ace;

/** Status codes of the C ABI ({@code AceStatus}, docs/NATIVE-INTEROP.md rule 1). */
public enum Status {
    /** The call succeeded. */
    OK(0),
    /** An argument is invalid: a null pointer, a bad size, a zero block size, a reversed range. */
    INVALID_ARGUMENT(1),
    /** The input does not start with the ACE signature. */
    NOT_ACE_FILE(2),
    /** The input uses a feature this build cannot read (a newer format, an unknown codec). */
    UNSUPPORTED(3),
    /** The data is damaged or hostile. */
    CORRUPT(4),
    /** A checksum does not match; the exception names the block when it is known. */
    CHECKSUM_MISMATCH(5),
    /** A decode limit was exceeded; nothing was allocated for the offending size. */
    LIMIT_EXCEEDED(6),
    /** A block number is outside the file. */
    BLOCK_NOT_FOUND(7),
    /** Reading or writing failed. */
    IO(8),
    /** A Rust panic was caught at the boundary (a bug). */
    PANIC(9),
    /** A state the library believes impossible (a bug). */
    INTERNAL(10),
    /** A caller-provided output buffer is too small. */
    BUFFER_TOO_SMALL(11),
    /** A status this SDK does not know; the library is newer than the SDK. */
    UNKNOWN(-1);

    private final int code;

    Status(int code) {
        this.code = code;
    }

    /** The numeric value of the C ABI. */
    public int code() {
        return code;
    }

    /** Maps a numeric status of the C ABI; unknown values map to {@link #UNKNOWN}. */
    public static Status of(int code) {
        for (Status status : values()) {
            if (status.code == code) {
                return status;
            }
        }
        return UNKNOWN;
    }
}
