package com.lingxia.webview;

import java.util.concurrent.atomic.AtomicLong;

/** Linearized native state for one WebView's top-level document transport. */
final class AndroidDocumentBridgeState {
    private static final AtomicLong NEXT_LOAD_TOKEN = new AtomicLong(1L);

    static final class Navigation {
        final long loadToken;
        final boolean trustedHostLoad;

        Navigation(long loadToken, boolean trustedHostLoad) {
            this.loadToken = loadToken;
            this.trustedHostLoad = trustedHostLoad;
        }
    }

    private long preparedLoadToken;
    private boolean preparedTrustedHostLoad;
    private long activeLoadToken;
    private boolean activeTrustedHostLoad;
    private long committedLoadToken;
    private long committedGeneration;
    private boolean historyObservedForActiveLoad;
    private long historyProofLoadToken;
    private long historyProofGeneration;
    private boolean historyReproofPending;
    // URL the active attempt last started; a redirect restart moves it. A
    // finished-load fallback may only bind the document this URL names.
    private String activeStartUrl;
    // Chromium finishes error pages too: an attempt whose main frame failed
    // never binds from its finish.
    private long mainFrameFailedLoadToken;

    static long nextLoadToken() {
        while (true) {
            long token = NEXT_LOAD_TOKEN.get();
            if (token <= 0L || token == Long.MAX_VALUE) {
                throw new IllegalStateException("Android load token space exhausted");
            }
            if (NEXT_LOAD_TOKEN.compareAndSet(token, token + 1L)) {
                return token;
            }
        }
    }

    synchronized void prepareHostLoad(long loadToken, boolean trustedHostLoad) {
        requirePositive(loadToken, "loadToken");
        preparedLoadToken = loadToken;
        preparedTrustedHostLoad = trustedHostLoad;
        activeLoadToken = 0L;
        activeTrustedHostLoad = false;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        activeStartUrl = null;
        mainFrameFailedLoadToken = 0L;
        clearHistoryEvidence();
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken) {
        return onPageStarted(fallbackLoadToken, null);
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken, String url) {
        requirePositive(fallbackLoadToken, "fallbackLoadToken");
        long previousLoadToken = activeLoadToken;
        boolean replacesCommittedDocument = committedLoadToken != 0L;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        if (preparedLoadToken != 0L) {
            activeLoadToken = preparedLoadToken;
            activeTrustedHostLoad = preparedTrustedHostLoad;
            preparedLoadToken = 0L;
            preparedTrustedHostLoad = false;
        } else if (activeLoadToken == 0L || replacesCommittedDocument) {
            activeLoadToken = fallbackLoadToken;
            activeTrustedHostLoad = false;
        } else if (activeTrustedHostLoad) {
            // Direct HTML has no legitimate redirect. A second start is a
            // distinct, untrusted renderer load and must not inherit either
            // its loader key or its attestation.
            activeLoadToken = fallbackLoadToken;
            activeTrustedHostLoad = false;
        }
        activeStartUrl = url;
        mainFrameFailedLoadToken = 0L;
        if (activeLoadToken != previousLoadToken) {
            clearHistoryEvidence();
        }
        return new Navigation(activeLoadToken, activeTrustedHostLoad);
    }

    synchronized Navigation pendingCommit() {
        if (activeLoadToken == 0L || committedLoadToken == activeLoadToken) {
            return null;
        }
        return new Navigation(activeLoadToken, activeTrustedHostLoad);
    }

    /**
     * Commit evidence from a finished main-frame load, for a document whose
     * visible commit Chromium skipped: a WebView covered by a splash, zero-size,
     * or off-screen like a preloaded tab.
     *
     * Narrower than {@link #pendingCommit()}. Never for the browser profile,
     * which keeps requiring visible-commit proof; never once the attempt is
     * bound or its main frame failed; never for about:blank; and only when the
     * finished URL is the one this attempt started, so a late finish for a
     * different document cannot hand this attempt's port to the page still on
     * screen. A late finish for the same URL (a page reloading itself) cannot
     * be told apart here; Chromium does not deliver it after the next start.
     */
    synchronized Navigation pendingFinishCommit(String finishedUrl, boolean browserProfile) {
        if (browserProfile
                || activeLoadToken == 0L
                || committedLoadToken == activeLoadToken
                || mainFrameFailedLoadToken == activeLoadToken
                || !sameDocumentUrl(activeStartUrl, finishedUrl)) {
            return null;
        }
        return new Navigation(activeLoadToken, activeTrustedHostLoad);
    }

    synchronized void recordMainFrameFailure(long loadToken) {
        if (loadToken != 0L && loadToken == activeLoadToken) {
            mainFrameFailedLoadToken = loadToken;
        }
    }

    synchronized boolean bindCommit(long loadToken, long generation) {
        requirePositive(generation, "generation");
        if (activeLoadToken != loadToken || loadToken == 0L) {
            return false;
        }
        committedLoadToken = loadToken;
        committedGeneration = generation;
        if (historyObservedForActiveLoad) {
            historyProofLoadToken = loadToken;
            historyProofGeneration = generation;
        }
        return true;
    }

    /**
     * A second visited-history signal for one committed trusted load has no
     * fresh start/commit proof. Treat it as a possible history/BFCache restore,
     * revoke the document immediately, and request exactly one trusted reproof.
     */
    synchronized boolean historyRestoreNeedsReproof(boolean browserProfile) {
        if (!browserProfile || !activeTrustedHostLoad || activeLoadToken == 0L) {
            return false;
        }
        if (historyReproofPending) {
            return false;
        }
        if (committedLoadToken == 0L || committedGeneration == 0L) {
            // Android may report visited history before onPageCommitVisible.
            // bindCommit will associate this signal with the fresh attempt.
            historyObservedForActiveLoad = true;
            return false;
        }
        if (historyProofLoadToken != committedLoadToken
                || historyProofGeneration != committedGeneration) {
            historyProofLoadToken = committedLoadToken;
            historyProofGeneration = committedGeneration;
            return false;
        }

        historyReproofPending = true;
        preparedLoadToken = 0L;
        preparedTrustedHostLoad = false;
        activeLoadToken = 0L;
        activeTrustedHostLoad = false;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        historyObservedForActiveLoad = false;
        historyProofLoadToken = 0L;
        historyProofGeneration = 0L;
        return true;
    }

    synchronized long currentLoadToken() {
        return activeLoadToken;
    }

    synchronized boolean mayInstallPort(
            long loadToken,
            long generation,
            boolean browserProfile
    ) {
        return acceptsPort(loadToken, generation)
                && (!browserProfile || activeTrustedHostLoad);
    }

    synchronized boolean acceptsPort(long loadToken, long generation) {
        return loadToken != 0L
                && committedLoadToken == loadToken
                && committedGeneration == generation;
    }

    synchronized boolean hasCommittedDocument() {
        return committedLoadToken != 0L && committedGeneration != 0L;
    }

    synchronized void revoke() {
        preparedLoadToken = 0L;
        preparedTrustedHostLoad = false;
        activeLoadToken = 0L;
        activeTrustedHostLoad = false;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        activeStartUrl = null;
        mainFrameFailedLoadToken = 0L;
        clearHistoryEvidence();
    }

    private void clearHistoryEvidence() {
        historyObservedForActiveLoad = false;
        historyProofLoadToken = 0L;
        historyProofGeneration = 0L;
        historyReproofPending = false;
    }

    /** The same document: equal URLs once any fragment is dropped. */
    private static boolean sameDocumentUrl(String started, String finished) {
        if (started == null || finished == null || started.isEmpty() || finished.isEmpty()) {
            return false;
        }
        String document = withoutFragment(finished);
        return !"about:blank".equals(document) && withoutFragment(started).equals(document);
    }

    private static String withoutFragment(String url) {
        int hash = url.indexOf('#');
        return hash < 0 ? url : url.substring(0, hash);
    }

    private static void requirePositive(long value, String name) {
        if (value <= 0L) {
            throw new IllegalArgumentException(name + " must be positive");
        }
    }
}
