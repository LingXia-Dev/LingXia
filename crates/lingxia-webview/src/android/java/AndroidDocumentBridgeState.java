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
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken) {
        requirePositive(fallbackLoadToken, "fallbackLoadToken");
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
    }

    private static void requirePositive(long value, String name) {
        if (value <= 0L) {
            throw new IllegalArgumentException(name + " must be positive");
        }
    }
}
