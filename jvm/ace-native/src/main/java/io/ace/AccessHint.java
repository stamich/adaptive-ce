package io.ace;

/** The expected way the file will be read; consulted by {@link BlockSizePolicy#AUTO}. */
public enum AccessHint {
    /** Long sequential scans: favour throughput. */
    SEQUENTIAL(0),
    /** A mix of scans and range reads. The default. */
    BALANCED(1),
    /** Point and range reads: cap the block size to protect latency. */
    RANDOM_ACCESS(2);

    private final int code;

    AccessHint(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }
}
