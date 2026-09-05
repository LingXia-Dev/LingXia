package com.lingxia.webview;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class WebMessageSizePolicyTest {
    @Test
    public void asciiBoundaryIsAcceptedAndOversizeIsRejected() {
        assertTrue(WebMessageSizePolicy.isWithinLimit(
                repeat("x", WebMessageSizePolicy.MAX_MESSAGE_BYTES)));
        assertFalse(WebMessageSizePolicy.isWithinLimit(
                repeat("x", WebMessageSizePolicy.MAX_MESSAGE_BYTES + 1)));
    }

    @Test
    public void utf8ByteLengthDoesNotUseUtf16CodeUnitLength() {
        assertTrue(WebMessageSizePolicy.isWithinLimit(repeat("é", 32 * 1024)));
        assertFalse(WebMessageSizePolicy.isWithinLimit(repeat("é", 32 * 1024 + 1)));
        assertTrue(WebMessageSizePolicy.isWithinLimit(repeat("😀", 16 * 1024)));
        assertFalse(WebMessageSizePolicy.isWithinLimit(repeat("😀", 16 * 1024 + 1)));
    }

    @Test
    public void malformedSurrogateCountingIsConservative() {
        assertTrue(WebMessageSizePolicy.isWithinLimit(repeat("\ud800", 21_845)));
        assertFalse(WebMessageSizePolicy.isWithinLimit(repeat("\ud800", 21_846)));
    }

    private static String repeat(String value, int count) {
        StringBuilder result = new StringBuilder(value.length() * count);
        for (int i = 0; i < count; i++) {
            result.append(value);
        }
        return result.toString();
    }
}
