package com.lingxia.webview;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class AndroidDocumentBridgeStateTest {
    @Test
    public void firstStartHasNoCommittedDocumentUntilBind() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        assertFalse(state.hasCommittedDocument());
        state.prepareHostLoad(11L, false);
        assertFalse(state.hasCommittedDocument());
        AndroidDocumentBridgeState.Navigation started = state.onPageStarted(100L);
        assertFalse(state.hasCommittedDocument());
        assertTrue(state.bindCommit(started.loadToken, 1L));
        assertTrue(state.hasCommittedDocument());
    }

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

    private static final String HOME = "lx://app/pages/home/index.html";

    @Test
    public void finishedLoadBindsStrictDocumentWhenVisibleCommitIsSkipped() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(101L, true);
        AndroidDocumentBridgeState.Navigation current = state.onPageStarted(120L, HOME);

        AndroidDocumentBridgeState.Navigation finish = state.pendingFinishCommit(HOME, false);
        assertNotNull(finish);
        assertTrue(finish.loadToken == current.loadToken);
        assertTrue(state.bindCommit(finish.loadToken, 9L));
        assertTrue(state.acceptsPort(current.loadToken, 9L));
        // Bound once: neither a late visible commit nor a second finish rebinds.
        assertNull(state.pendingCommit());
        assertNull(state.pendingFinishCommit(HOME, false));
    }

    @Test
    public void visibleCommitLeavesNothingForTheFinishFallback() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(102L, true);
        AndroidDocumentBridgeState.Navigation current = state.onPageStarted(121L, HOME);

        assertTrue(state.bindCommit(state.pendingCommit().loadToken, 10L));
        assertNull(state.pendingFinishCommit(HOME, false));
        assertTrue(state.acceptsPort(current.loadToken, 10L));
    }

    @Test
    public void browserProfileNeverBindsFromAFinishedLoad() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(103L, true);
        state.onPageStarted(122L, "https://example.com/");

        assertNull(state.pendingFinishCommit("https://example.com/", true));
        assertNotNull(state.pendingCommit());
    }

    @Test
    public void lateFinishForAReplacedDocumentCannotBindTheNewAttempt() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(104L, true);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(123L, "lx://app/a");
        assertTrue(state.bindCommit(first.loadToken, 11L));

        state.prepareHostLoad(105L, true);
        // Nothing is active between the host load and its start.
        assertNull(state.pendingFinishCommit("lx://app/a", false));
        AndroidDocumentBridgeState.Navigation next = state.onPageStarted(124L, "lx://app/b");
        // The old document's finish names the old URL.
        assertNull(state.pendingFinishCommit("lx://app/a", false));
        AndroidDocumentBridgeState.Navigation finish = state.pendingFinishCommit("lx://app/b", false);
        assertNotNull(finish);
        assertTrue(finish.loadToken == next.loadToken);
    }

    @Test
    public void sameUrlReloadWaitsForItsOwnStart() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(106L, true);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(125L, HOME);
        assertTrue(state.bindCommit(first.loadToken, 12L));

        // WebView.reload() / a dev bundle reload prepares a fresh attempt.
        state.prepareHostLoad(107L, false);
        assertNull(state.pendingFinishCommit(HOME, false));
        AndroidDocumentBridgeState.Navigation reload = state.onPageStarted(126L, HOME);

        AndroidDocumentBridgeState.Navigation finish = state.pendingFinishCommit(HOME, false);
        assertNotNull(finish);
        assertTrue(state.bindCommit(finish.loadToken, 13L));
        assertFalse(state.acceptsPort(first.loadToken, 12L));
        assertTrue(state.acceptsPort(reload.loadToken, 13L));
    }

    @Test
    public void pageReloadingItselfIsANewAttemptJudgedOnItsOwnFinish() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(108L, true);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(127L, HOME);
        assertTrue(state.bindCommit(first.loadToken, 14L));

        // location.reload(): no host load, just a new start over a bound document.
        AndroidDocumentBridgeState.Navigation reload = state.onPageStarted(128L, HOME);
        assertFalse(reload.loadToken == first.loadToken);
        assertFalse(state.acceptsPort(first.loadToken, 14L));
        AndroidDocumentBridgeState.Navigation finish = state.pendingFinishCommit(HOME, false);
        assertNotNull(finish);
        assertTrue(finish.loadToken == reload.loadToken);
    }

    @Test
    public void failedMainFrameLoadNeverBindsOnFinish() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(109L, true);
        AndroidDocumentBridgeState.Navigation failed = state.onPageStarted(129L, HOME);
        state.recordMainFrameFailure(failed.loadToken);
        assertNull(state.pendingFinishCommit(HOME, false));

        // A fresh attempt is judged on its own evidence.
        state.prepareHostLoad(110L, true);
        state.onPageStarted(130L, HOME);
        assertNotNull(state.pendingFinishCommit(HOME, false));
    }

    @Test
    public void staleFailureReportCannotPoisonTheCurrentAttempt() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(111L, true);
        AndroidDocumentBridgeState.Navigation old = state.onPageStarted(131L, "lx://app/a");
        state.prepareHostLoad(112L, true);
        state.onPageStarted(132L, HOME);

        state.recordMainFrameFailure(old.loadToken);
        assertNotNull(state.pendingFinishCommit(HOME, false));
    }

    @Test
    public void redirectRestartMovesTheExpectedUrl() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(113L, false);
        AndroidDocumentBridgeState.Navigation first = state.onPageStarted(133L, "lx://app/start");
        AndroidDocumentBridgeState.Navigation redirect = state.onPageStarted(134L, "lx://app/final");

        assertTrue(first.loadToken == redirect.loadToken);
        assertNull(state.pendingFinishCommit("lx://app/start", false));
        assertNotNull(state.pendingFinishCommit("lx://app/final", false));
    }

    @Test
    public void fragmentKeepsTheDocumentAndBlankNeverBinds() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(114L, true);
        state.onPageStarted(135L, HOME);
        assertNotNull(state.pendingFinishCommit(HOME + "#top", false));

        state.prepareHostLoad(115L, true);
        state.onPageStarted(136L, "about:blank");
        assertNull(state.pendingFinishCommit("about:blank", false));
    }

    @Test
    public void teardownLeavesNothingToBindOnFinish() {
        AndroidDocumentBridgeState state = new AndroidDocumentBridgeState();
        state.prepareHostLoad(116L, true);
        state.onPageStarted(137L, HOME);

        state.revoke();

        assertNull(state.pendingFinishCommit(HOME, false));
    }
}
