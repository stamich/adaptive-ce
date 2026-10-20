package io.ace;

/**
 * The result of {@link Engine#compressWithStats(byte[])}.
 *
 * @param bytes the ACE file
 * @param stats what the run did
 */
public record Compressed(byte[] bytes, Stats stats) {}
