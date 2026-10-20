package io.ace;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.SegmentAllocator;
import java.lang.foreign.ValueLayout;
import io.ace.ffi.AceBindings;
import java.util.Objects;

/**
 * Compression settings. Immutable; create with {@link #builder()} or {@link #defaults()}.
 *
 * <p>Zero for {@code blockSize}, {@code threads} and {@code memoryBudget} keeps the library
 * default (256 KiB blocks, one worker per core, 256 MiB).
 */
public final class Config {
    private static final Config DEFAULTS = builder().build();

    private final Profile profile;
    private final long blockSize;
    private final long threads;
    private final long memoryBudget;
    private final boolean writeIndex;
    private final BlockSizePolicy blockSizePolicy;
    private final AccessHint accessHint;
    private final boolean numericSpecialization;
    private final boolean floatSpecialization;

    private Config(Builder b) {
        this.profile = b.profile;
        this.blockSize = b.blockSize;
        this.threads = b.threads;
        this.memoryBudget = b.memoryBudget;
        this.writeIndex = b.writeIndex;
        this.blockSizePolicy = b.blockSizePolicy;
        this.accessHint = b.accessHint;
        this.numericSpecialization = b.numericSpecialization;
        this.floatSpecialization = b.floatSpecialization;
    }

    /** The default settings. */
    public static Config defaults() {
        return DEFAULTS;
    }

    /** Starts a builder initialised with the defaults. */
    public static Builder builder() {
        return new Builder();
    }

    /** Encoder effort. */
    public Profile profile() {
        return profile;
    }

    /** Block size in bytes; 0 keeps the library default. */
    public long blockSize() {
        return blockSize;
    }

    /** Worker threads; 0 means one per available core. */
    public long threads() {
        return threads;
    }

    /** Soft memory cap in bytes; 0 keeps the library default. */
    public long memoryBudget() {
        return memoryBudget;
    }

    /** Whether the block index is written; range reads need it. */
    public boolean writeIndex() {
        return writeIndex;
    }

    /** Fixed or automatic block size. */
    public BlockSizePolicy blockSizePolicy() {
        return blockSizePolicy;
    }

    /** Expected access pattern. */
    public AccessHint accessHint() {
        return accessHint;
    }

    /** Whether the integer pipelines are enabled. */
    public boolean numericSpecialization() {
        return numericSpecialization;
    }

    /** Whether the floating-point pipelines are enabled. */
    public boolean floatSpecialization() {
        return floatSpecialization;
    }

    /** Writes this configuration as an {@code AceConfig} struct. */
    MemorySegment toNative(SegmentAllocator allocator) {
        MemorySegment config = allocator.allocate(AceBindings.ACE_CONFIG);
        config.set(ValueLayout.JAVA_LONG, AceBindings.ACE_CONFIG_STRUCT_SIZE, AceBindings.ACE_CONFIG_SIZE);
        config.set(ValueLayout.JAVA_INT, AceBindings.ACE_CONFIG_PROFILE, profile.code());
        config.set(ValueLayout.JAVA_LONG, AceBindings.ACE_CONFIG_BLOCK_SIZE, blockSize);
        config.set(ValueLayout.JAVA_LONG, AceBindings.ACE_CONFIG_THREADS, threads);
        config.set(ValueLayout.JAVA_LONG, AceBindings.ACE_CONFIG_MEMORY_BUDGET, memoryBudget);
        config.set(ValueLayout.JAVA_INT, AceBindings.ACE_CONFIG_BLOCK_SIZE_POLICY, blockSizePolicy.code());
        config.set(ValueLayout.JAVA_INT, AceBindings.ACE_CONFIG_ACCESS_HINT, accessHint.code());
        config.set(ValueLayout.JAVA_BYTE, AceBindings.ACE_CONFIG_WRITE_INDEX, (byte) (writeIndex ? 1 : 0));
        config.set(ValueLayout.JAVA_BYTE, AceBindings.ACE_CONFIG_NUMERIC_SPECIALIZATION,
                (byte) (numericSpecialization ? 1 : 0));
        config.set(ValueLayout.JAVA_BYTE, AceBindings.ACE_CONFIG_FLOAT_SPECIALIZATION,
                (byte) (floatSpecialization ? 1 : 0));
        return config;
    }

    @Override
    public String toString() {
        return "Config[profile=" + profile + ", blockSize=" + blockSize + ", threads=" + threads
                + ", memoryBudget=" + memoryBudget + ", writeIndex=" + writeIndex + ", blockSizePolicy="
                + blockSizePolicy + ", accessHint=" + accessHint + ", numeric=" + numericSpecialization
                + ", float=" + floatSpecialization + "]";
    }

    /** Builds a {@link Config}. */
    public static final class Builder {
        private Profile profile = Profile.BALANCED;
        private long blockSize;
        private long threads;
        private long memoryBudget;
        private boolean writeIndex = true;
        private BlockSizePolicy blockSizePolicy = BlockSizePolicy.FIXED;
        private AccessHint accessHint = AccessHint.BALANCED;
        private boolean numericSpecialization = true;
        private boolean floatSpecialization = true;

        private Builder() {}

        /** Encoder effort (default {@link Profile#BALANCED}). */
        public Builder profile(Profile value) {
            this.profile = Objects.requireNonNull(value, "profile");
            return this;
        }

        /** Block size in bytes; 0 keeps the library default. */
        public Builder blockSize(long bytes) {
            this.blockSize = nonNegative(bytes, "blockSize");
            return this;
        }

        /** Worker threads; 0 means one per available core. The output does not depend on it. */
        public Builder threads(long count) {
            this.threads = nonNegative(count, "threads");
            return this;
        }

        /** Soft cap on the memory of blocks in flight, in bytes; 0 keeps the library default. */
        public Builder memoryBudget(long bytes) {
            this.memoryBudget = nonNegative(bytes, "memoryBudget");
            return this;
        }

        /** Writes the block index and trailer (default true); range reads need it. */
        public Builder writeIndex(boolean write) {
            this.writeIndex = write;
            return this;
        }

        /** Fixed or automatic block size (default {@link BlockSizePolicy#FIXED}). */
        public Builder blockSizePolicy(BlockSizePolicy value) {
            this.blockSizePolicy = Objects.requireNonNull(value, "blockSizePolicy");
            return this;
        }

        /** Expected access pattern (default {@link AccessHint#BALANCED}). */
        public Builder accessHint(AccessHint value) {
            this.accessHint = Objects.requireNonNull(value, "accessHint");
            return this;
        }

        /** Enables the integer pipelines (default true). */
        public Builder numericSpecialization(boolean enabled) {
            this.numericSpecialization = enabled;
            return this;
        }

        /** Enables the floating-point pipelines (default true). */
        public Builder floatSpecialization(boolean enabled) {
            this.floatSpecialization = enabled;
            return this;
        }

        /** Creates the configuration; the native engine validates it when it is created. */
        public Config build() {
            return new Config(this);
        }
    }

    static long nonNegative(long value, String name) {
        if (value < 0) {
            throw new IllegalArgumentException(name + " must not be negative: " + value);
        }
        return value;
    }
}
