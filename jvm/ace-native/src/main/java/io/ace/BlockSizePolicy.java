package io.ace;

/** Whether the block size is used as given or chosen once per file. */
public enum BlockSizePolicy {
    /** Use the configured block size exactly. The default. */
    FIXED(0),
    /** Choose one deterministic block size for the whole file, guided by the {@link AccessHint}. */
    AUTO(1);

    private final int code;

    BlockSizePolicy(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }
}
