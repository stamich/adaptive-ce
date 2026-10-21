package io.ace;

import io.ace.ffi.AceBindings;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SegmentAllocator;
import java.lang.foreign.ValueLayout;

/**
 * Upper bounds a decoder enforces on everything a file declares. Zero keeps the library default
 * of that limit. A limit only rejects input (with {@link Status#LIMIT_EXCEEDED}), before anything
 * of the offending size is allocated.
 *
 * <p>Lower them when decoding untrusted input on a small machine.
 */
public final class DecodeLimits {
    private static final DecodeLimits DEFAULTS = builder().build();

    private final long maxOutputSize;
    private final long maxBlockSize;
    private final long maxEncodedBlockSize;
    private final long maxTransforms;
    private final long maxDictionarySize;
    private final long maxBlocks;

    private DecodeLimits(Builder b) {
        this.maxOutputSize = b.maxOutputSize;
        this.maxBlockSize = b.maxBlockSize;
        this.maxEncodedBlockSize = b.maxEncodedBlockSize;
        this.maxTransforms = b.maxTransforms;
        this.maxDictionarySize = b.maxDictionarySize;
        this.maxBlocks = b.maxBlocks;
    }

    /** The library defaults. */
    public static DecodeLimits defaults() {
        return DEFAULTS;
    }

    /** Starts a builder in which every limit keeps the library default. */
    public static Builder builder() {
        return new Builder();
    }

    /** Maximum total decoded output in bytes (0 = default). */
    public long maxOutputSize() {
        return maxOutputSize;
    }

    /** Maximum decoded size of one block in bytes (0 = default). */
    public long maxBlockSize() {
        return maxBlockSize;
    }

    /** Maximum encoded size of one block in bytes (0 = default). */
    public long maxEncodedBlockSize() {
        return maxEncodedBlockSize;
    }

    /** Maximum number of transforms of one block (0 = default). */
    public long maxTransforms() {
        return maxTransforms;
    }

    /** Maximum embedded dictionary size in bytes (0 = default). */
    public long maxDictionarySize() {
        return maxDictionarySize;
    }

    /** Maximum number of blocks in a file (0 = default). */
    public long maxBlocks() {
        return maxBlocks;
    }

    /** Writes the limits as an {@code AceLimits} struct. */
    MemorySegment toNative(SegmentAllocator allocator) {
        MemorySegment limits = allocator.allocate(AceBindings.ACE_LIMITS);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_STRUCT_SIZE, AceBindings.ACE_LIMITS_SIZE);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_OUTPUT_SIZE, maxOutputSize);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_BLOCK_SIZE, maxBlockSize);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_ENCODED_BLOCK_SIZE, maxEncodedBlockSize);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_TRANSFORMS, maxTransforms);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_DICTIONARY_SIZE, maxDictionarySize);
        limits.set(ValueLayout.JAVA_LONG, AceBindings.ACE_LIMITS_MAX_BLOCKS, maxBlocks);
        return limits;
    }

    /** Builds {@link DecodeLimits}. */
    public static final class Builder {
        private long maxOutputSize;
        private long maxBlockSize;
        private long maxEncodedBlockSize;
        private long maxTransforms;
        private long maxDictionarySize;
        private long maxBlocks;

        private Builder() {}

        /** Maximum total decoded output in bytes. */
        public Builder maxOutputSize(long bytes) {
            this.maxOutputSize = Config.nonNegative(bytes, "maxOutputSize");
            return this;
        }

        /** Maximum decoded size of one block in bytes. */
        public Builder maxBlockSize(long bytes) {
            this.maxBlockSize = Config.nonNegative(bytes, "maxBlockSize");
            return this;
        }

        /** Maximum encoded size of one block in bytes. */
        public Builder maxEncodedBlockSize(long bytes) {
            this.maxEncodedBlockSize = Config.nonNegative(bytes, "maxEncodedBlockSize");
            return this;
        }

        /** Maximum number of transforms of one block. */
        public Builder maxTransforms(long count) {
            this.maxTransforms = Config.nonNegative(count, "maxTransforms");
            return this;
        }

        /** Maximum embedded dictionary size in bytes. */
        public Builder maxDictionarySize(long bytes) {
            this.maxDictionarySize = Config.nonNegative(bytes, "maxDictionarySize");
            return this;
        }

        /** Maximum number of blocks in a file. */
        public Builder maxBlocks(long count) {
            this.maxBlocks = Config.nonNegative(count, "maxBlocks");
            return this;
        }

        /** Creates the limits. */
        public DecodeLimits build() {
            return new DecodeLimits(this);
        }
    }
}
