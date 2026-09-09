package com.lingxia.webview;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class AndroidDocumentBridgeStateTest {
    @Test
    public void staleNavigationCannotCommitOrReuseSuccessorPort() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(11L, true);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(100L);

        state.prepareHostLoad(12L, true);
        AndroidDocumentBridgeState.Navigation second = state.onPageStarted(101L);

        assertFalse(state.bindCommit(first.loadToken, 1L));
        assertTrue(state.bindCommit(second.loadToken, 2L));
        assertFalse(state.acceptsPort(first.loadToken, 1L));
        assertTrue(state.acceptsPort(second.loadToken, 2L));
    }

    @Test
    public void externalDocumentNeverInstallsBrowserControlPort() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(21L, false);
        AndroidDocumentBridgeState.Navigation external = state.onPageStarted(102L);

        assertTrue(state.bindCommit(external.loadToken, 1L));
        assertFalse(state.mayInstallPort(external.loadToken, 1L, true));
    }

    @Test
    public void reloadRevokesOldPortAndCreatesANewAttempt() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(31L, true);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(103L);
        assertTrue(state.bindCommit(first.loadToken, 1L));

        state.prepareHostLoad(32L, false);
        assertFalse(state.acceptsPort(first.loadToken, 1L));
        AndroidDocumentBridgeState.Navigation reload = state.onPageStarted(103L);

        assertFalse(state.acceptsPort(first.loadToken, 1L));
        assertTrue(state.bindCommit(reload.loadToken, 2L));
        assertFalse(state.mayInstallPort(reload.loadToken, 2L, true));
    }

    @Test
    public void repeatedRedirectStartReusesOneAttemptUntilCommit() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(41L, false);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(104L);
        AndroidDocumentBridgeState.Navigation redirect = state.onPageStarted(105L);

        assertTrue(first.loadToken == redirect.loadToken);
        assertNotNull(state.pendingCommit());
        assertTrue(state.bindCommit(redirect.loadToken, 1L));
        assertFalse(state.mayInstallPort(redirect.loadToken, 1L, true));
        assertNull(state.pendingCommit());
    }

    @Test
    public void externalStartCannotInheritDirectLoaderAttestation() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(45L, true);
        AndroidDocumentBridgeState.Navigation direct = state.onPageStarted(104L);

        AndroidDocumentBridgeState.Navigation external = state.onPageStarted(105L);

        assertFalse(direct.loadToken == external.loadToken);
        assertFalse(state.bindCommit(direct.loadToken, 1L));
        assertTrue(state.bindCommit(external.loadToken, 1L));
        assertFalse(state.mayInstallPort(external.loadToken, 1L, true));
    }

    @Test
    public void crashOrTeardownRevokesPortAndPendingCommit() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(51L, true);
        AndroidDocumentBridgeState.Navigation current = state.onPageStarted(106L);
        assertTrue(state.bindCommit(current.loadToken, 1L));

        state.revoke();

        assertFalse(state.acceptsPort(current.loadToken, 1L));
        assertNull(state.pendingCommit());
    }

    @Test
    public void sameUrlHistoryRestoreWithoutCallbacksRevokesOldPortOnce() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(61L, true);
        AndroidDocumentBridgeState.Navigation current = state.onPageStarted(107L);
        assertTrue(state.bindCommit(current.loadToken, 3L));

        assertFalse(state.historyRestoreNeedsReproof(true));
        assertTrue(state.acceptsPort(current.loadToken, 3L));
        // No new start/commit callbacks: a same-URL restore reports history
        // against the already consumed proof.
        assertTrue(state.historyRestoreNeedsReproof(true));
        assertFalse(state.acceptsPort(current.loadToken, 3L));
        assertFalse(state.historyRestoreNeedsReproof(true));
    }

    @Test
    public void visitedHistoryBeforeCommitConsumesTheFreshProof() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(71L, true);
        AndroidDocumentBridgeState.Navigation current = state.onPageStarted(108L);

        assertFalse(state.historyRestoreNeedsReproof(true));
        assertTrue(state.bindCommit(current.loadToken, 4L));
        assertTrue(state.acceptsPort(current.loadToken, 4L));
        assertTrue(state.historyRestoreNeedsReproof(true));
        assertFalse(state.acceptsPort(current.loadToken, 4L));
    }

    @Test
    public void freshTrustedReloadGetsNewProofAfterRestoreRevocation() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(81L, true);
        AndroidDocumentBridgeState.Navigation old = state.onPageStarted(109L);
        assertTrue(state.bindCommit(old.loadToken, 5L));
        assertFalse(state.historyRestoreNeedsReproof(true));
        assertTrue(state.historyRestoreNeedsReproof(true));

        state.prepareHostLoad(82L, true);
        AndroidDocumentBridgeState.Navigation fresh = state.onPageStarted(110L);
        assertTrue(state.bindCommit(fresh.loadToken, 6L));
        assertFalse(state.acceptsPort(old.loadToken, 5L));
        assertTrue(state.acceptsPort(fresh.loadToken, 6L));
        assertFalse(state.historyRestoreNeedsReproof(true));
    }

    @Test
    public void ordinaryOrExternalDocumentsIgnoreHistoryReproofPolicy() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(91L, false);
        AndroidDocumentBridgeState.Navigation external = state.onPageStarted(111L);
        assertTrue(state.bindCommit(external.loadToken, 7L));

        assertFalse(state.historyRestoreNeedsReproof(true));
        assertFalse(state.historyRestoreNeedsReproof(true));

        state.prepareHostLoad(92L, true);
        AndroidDocumentBridgeState.Navigation ordinary = state.onPageStarted(112L);
        assertTrue(state.bindCommit(ordinary.loadToken, 8L));
        assertFalse(state.historyRestoreNeedsReproof(false));
        assertFalse(state.historyRestoreNeedsReproof(false));
        assertTrue(state.acceptsPort(ordinary.loadToken, 8L));
    }
}
