package com.lingxia.webview;

/** Fail-closed Android BrowserControl transport policy and metric reasons. */
final class BrowserControlBridgePolicy {
    static final String REASON_API_BELOW_23 = "android_api_below_23";
    static final String REASON_MESSAGE_PORT_UNAVAILABLE = "android_message_port_unavailable";

    private BrowserControlBridgePolicy() {}

    static String degradationReason(int sdkInt, boolean messagePortSafe) {
        if (sdkInt < 23) {
            return REASON_API_BELOW_23;
        }
        return messagePortSafe ? null : REASON_MESSAGE_PORT_UNAVAILABLE;
    }
}
