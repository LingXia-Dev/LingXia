package com.lingxia.webview;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class DocumentCommitCallbackPolicyTest {
    @Test
    public void api21And22CannotBindDocumentFromPageFinished() {
        assertFalse(DocumentCommitCallbackPolicy.canBindDocument(21));
        assertFalse(DocumentCommitCallbackPolicy.canBindDocument(22));
        assertEquals(
                BrowserControlBridgePolicy.REASON_API_BELOW_23,
                BrowserControlBridgePolicy.degradationReason(21, false));
        assertEquals(
                BrowserControlBridgePolicy.REASON_API_BELOW_23,
                BrowserControlBridgePolicy.degradationReason(22, false));
    }

    @Test
    public void api23UsesCommitVisibleEvidence() {
        assertTrue(DocumentCommitCallbackPolicy.canBindDocument(23));
        assertNull(BrowserControlBridgePolicy.degradationReason(23, true));
    }
}
