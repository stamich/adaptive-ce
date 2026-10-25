package io.ace;

/**
 * What a {@link Column} holds. The kind steers which codec ACE tries for the column; it never
 * changes how the file is decoded.
 */
public enum ColumnKind {
    /** Opaque bytes of any width: identifiers, text, flags. Only the generic codecs are tried. */
    BYTES(0),
    /** Unsigned little-endian integers of width 1, 2, 4 or 8: counters, ids, timestamps. */
    UNSIGNED(1),
    /** Signed little-endian integers of width 1, 2, 4 or 8: deltas, offsets. */
    SIGNED(2),
    /** IEEE-754 little-endian floats of width 4 or 8: measurements, prices. */
    FLOAT(3);

    private final int code;

    ColumnKind(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }
}
