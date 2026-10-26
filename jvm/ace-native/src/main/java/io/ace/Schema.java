package io.ace;

import io.ace.ffi.AceBindings;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.List;

/**
 * The record layout of a table: a list of {@link Column}s whose concatenation is one record. A
 * file that is a table of fixed-size records is described by listing its fields; ACE then also
 * tries storing every block column by column, which usually compresses much better than row by
 * row (a counter column becomes a run of small deltas, a price column a run of similar floats).
 *
 * <p>ACE never guesses a schema. Without one the file is stored row-wise. A wrong schema costs
 * time but never space, because a block keeps the columnar form only when it is smaller. Readers
 * need no schema.
 *
 * <pre>{@code
 * Schema trades = Schema.of(Column.unsigned(8), Column.floating(8), Column.unsigned(4));
 * Config config = Config.builder().schema(trades).build();
 * }</pre>
 *
 * @param columns the columns in record order
 */
public record Schema(List<Column> columns) {
    /** Copies the list and rejects an empty one. */
    public Schema {
        columns = List.copyOf(columns);
        if (columns.isEmpty()) {
            throw new IllegalArgumentException("a schema needs at least one column");
        }
    }

    /** A schema of the given columns. */
    public static Schema of(Column... columns) {
        return new Schema(List.of(columns));
    }

    /** Size of one record in bytes. */
    public long recordSize() {
        long total = 0;
        for (Column column : columns) {
            total += column.width();
        }
        return total;
    }

    /**
     * Creates a native handle and registers its release in {@code temporaries}. The library checks
     * the record size against the format's limit.
     */
    MemorySegment toNative(Arena arena, Temporaries temporaries) {
        MemorySegment array = arena.allocate(AceBindings.ACE_COLUMN_SIZE * columns.size(), 4);
        for (int i = 0; i < columns.size(); i++) {
            long base = i * AceBindings.ACE_COLUMN_SIZE;
            Column column = columns.get(i);
            array.set(ValueLayout.JAVA_INT, base + AceBindings.ACE_COLUMN_KIND, column.kind().code());
            array.set(ValueLayout.JAVA_INT, base + AceBindings.ACE_COLUMN_WIDTH, column.width());
        }
        MemorySegment out = Native.slot(arena);
        Native.check(Native.fn().aceSchemaNew(array, columns.size(), out));
        MemorySegment handle = Native.pointerIn(out);
        temporaries.onClose(() -> Native.fn().aceSchemaFree(handle));
        return handle;
    }
}
