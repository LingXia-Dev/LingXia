package com.lingxia.webview;

/** Only API 23+ supplies visible main-frame document commit evidence. */
final class DocumentCommitCallbackPolicy {
    private DocumentCommitCallbackPolicy() {}

    static boolean canBindDocument(int sdkInt) {
        return sdkInt >= 23;
    }
}
