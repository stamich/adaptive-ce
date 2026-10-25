package io.ace;

import java.util.Objects;

/**
 * One field of a fixed-size record: how many bytes it takes and what it holds.
 *
 * @param kind what the column holds
 * @param width bytes per record: 1 to 65 535 for {@link ColumnKind#BYTES}; 1, 2, 4 or 8 for
 *     integers; 4 or 8 for floats
 */
public record Column(ColumnKind kind, int width) {
    /** Checks the value ranges that do not depend on the native library. */
    public Column {
        Objects.requireNonNull(kind, "kind");
        if (width < 1 || width > 65_535) {
            throw new IllegalArgumentException("column width must be 1 to 65535: " + width);
        }
        boolean valid = switch (kind) {
            case BYTES -> true;
            case UNSIGNED, SIGNED -> width == 1 || width == 2 || width == 4 || width == 8;
            case FLOAT -> width == 4 || width == 8;
        };
        if (!valid) {
            throw new IllegalArgumentException(kind + " columns cannot be " + width + " bytes wide");
        }
    }

    /** Opaque bytes, {@code width} per record. */
    public static Column bytes(int width) {
        return new Column(ColumnKind.BYTES, width);
    }

    /** Unsigned little-endian integers of width 1, 2, 4 or 8. */
    public static Column unsigned(int width) {
        return new Column(ColumnKind.UNSIGNED, width);
    }

    /** Signed little-endian integers of width 1, 2, 4 or 8. */
    public static Column signed(int width) {
        return new Column(ColumnKind.SIGNED, width);
    }

    /** IEEE-754 little-endian floats of width 4 or 8. */
    public static Column floating(int width) {
        return new Column(ColumnKind.FLOAT, width);
    }
}
