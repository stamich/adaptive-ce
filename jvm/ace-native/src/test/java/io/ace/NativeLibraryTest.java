package io.ace;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.ace.ffi.AceBindings;
import org.junit.jupiter.api.Test;

class NativeLibraryTest {
    @Test
    void abiVersionMatchesTheGeneratedBindings() {
        assertEquals(AceBindings.ABI_VERSION, NativeLibrary.abiVersion());
    }

    @Test
    void engineVersionIsTheProjectVersion() {
        assertEquals("0.7.0", NativeLibrary.engineVersion());
    }

    @Test
    void reportsWhereTheLibraryCameFrom() {
        assertFalse(NativeLibrary.origin().isEmpty());
    }

    @Test
    void layoutsMatchTheHeader() {
        assertEquals(56, AceBindings.ACE_CONFIG_SIZE);
        assertEquals(56, AceBindings.ACE_LIMITS_SIZE);
        assertEquals(AceBindings.ACE_CONFIG_SIZE, AceBindings.ACE_CONFIG.byteSize());
        assertTrue(AceBindings.ACE_CONFIG_RESERVED + 5 <= AceBindings.ACE_CONFIG_SIZE);
    }
}
