package com.lingxia.webview;

/** Allocation-free UTF-8 size gate shared by Android bridge transports. */
final class WebMessageSizePolicy {
    static final int MAX_MESSAGE_BYTES = 64 * 1024;

    private WebMessageSizePolicy() {}

    static boolean isWithinLimit(String value) {
        if (value == null) {
            return true;
        }
        int bytes = 0;
        for (int i = 0; i < value.length(); i++) {
            char current = value.charAt(i);
            int width;
            if (current <= 0x7f) {
                width = 1;
            } else if (current <= 0x7ff) {
                width = 2;
            } else if (Character.isHighSurrogate(current)
                    && i + 1 < value.length()
                    && Character.isLowSurrogate(value.charAt(i + 1))) {
                width = 4;
                i++;
            } else {
                // A lone surrogate is invalid Unicode. Counting its maximum
                // UTF-8 replacement width is conservative and allocation-free.
                width = 3;
            }
            if (bytes > MAX_MESSAGE_BYTES - width) {
                return false;
            }
            bytes += width;
        }
        return true;
    }
}
