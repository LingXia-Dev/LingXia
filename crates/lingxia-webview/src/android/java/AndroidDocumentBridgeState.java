package com.lingxia.webview;

import java.util.concurrent.atomic.AtomicLong;

/** Linearized native state for one WebView's top-level document transport. */
final class AndroidDocumentBridgeState {
    private static final AtomicLong NEXT_LOAD_TOKEN = new AtomicLong(1L);

    static final class Navigation {
        final long loadToken;

        Navigation(long loadToken) {
            this.loadToken = loadToken;
        }
    }

    private long preparedLoadToken;
    private long activeLoadToken;
    private long committedLoadToken;
    private long committedGeneration;
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

    synchronized void prepareHostLoad(long loadToken) {
        requirePositive(loadToken, "loadToken");
        preparedLoadToken = loadToken;
        activeLoadToken = 0L;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        activeStartUrl = null;
        mainFrameFailedLoadToken = 0L;
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken) {
        return onPageStarted(fallbackLoadToken, null);
    }

    synchronized Navigation onPageStarted(long fallbackLoadToken, String url) {
        requirePositive(fallbackLoadToken, "fallbackLoadToken");
        boolean replacesCommittedDocument = committedLoadToken != 0L;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        if (preparedLoadToken != 0L) {
            activeLoadToken = preparedLoadToken;
            preparedLoadToken = 0L;
        } else if (activeLoadToken == 0L || replacesCommittedDocument) {
            activeLoadToken = fallbackLoadToken;
        }
        activeStartUrl = url;
        mainFrameFailedLoadToken = 0L;
        return new Navigation(activeLoadToken);
    }

    synchronized Navigation pendingCommit() {
        if (activeLoadToken == 0L || committedLoadToken == activeLoadToken) {
            return null;
        }
        return new Navigation(activeLoadToken);
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
        return new Navigation(activeLoadToken);
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
                && !browserProfile;
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
        activeLoadToken = 0L;
        committedLoadToken = 0L;
        committedGeneration = 0L;
        activeStartUrl = null;
        mainFrameFailedLoadToken = 0L;
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
