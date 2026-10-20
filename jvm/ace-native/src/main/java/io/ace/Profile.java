package io.ace;

/** How much encoder effort to spend to make the output smaller. */
public enum Profile {
    /** Least analysis and encoding CPU cost. */
    FAST(0),
    /** Balances size and CPU cost. The default. */
    BALANCED(1),
    /** Searches more candidates; up to about 3 % smaller on some data, not guaranteed. */
    DENSE(2);

    private final int code;

    Profile(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }
}
