package com.lingxia.webview;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class DocumentCommitCallbackPolicyTest {
    @Test
    public void api21And22CannotBindDocumentFromPageFinished() {
        assertFalse(DocumentCommitCallbackPolicy.canBindDocument(21));
        assertFalse(DocumentCommitCallbackPolicy.canBindDocument(22));
    }

    @Test
    public void api23UsesCommitVisibleEvidence() {
        assertTrue(DocumentCommitCallbackPolicy.canBindDocument(23));
    }
}
