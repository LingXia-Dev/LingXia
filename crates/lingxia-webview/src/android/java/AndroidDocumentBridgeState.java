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
        clearHistoryEvidence();
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken) {
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

    synchronized void revoke() {
        preparedLoadToken = 0L;
        preparedTrustedHostLoad = false;
        activeLoadToken = 0L;
        activeTrustedHostLoad = false;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        clearHistoryEvidence();
    }

    private void clearHistoryEvidence() {
        historyObservedForActiveLoad = false;
        historyProofLoadToken = 0L;
        historyProofGeneration = 0L;
        historyReproofPending = false;
    }

    private static void requirePositive(long value, String name) {
        if (value <= 0L) {
            throw new IllegalArgumentException(name + " must be positive");
        }
    }
}
