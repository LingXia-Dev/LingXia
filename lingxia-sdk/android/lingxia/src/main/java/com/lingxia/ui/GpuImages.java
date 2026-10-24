package com.lingxia.ui;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Owner operations run on main; bitmap application runs on the View worker. */
final class GpuImages implements AutoCloseable {
    static final class Request {
        final String source;
        final String retryKey;
        final long token;
        Request(String source, String retryKey, long token) { this.source = source; this.retryKey = retryKey; this.token = token; }
    }
    private volatile long queue;
    private boolean closed;
    private final java.util.Map<Request, Runnable> cancellations = new java.util.HashMap<>();

    void clear() {
        if (queue != 0) GpuView.nativeImageQueueReset(queue, false);
        cancelObsolete();
    }
    void update(JSONArray sources) {
        if (closed || (queue == 0 && sources.length() == 0)) return;
        if (queue == 0) queue = GpuView.nativeImageQueueCreate();
        GpuView.nativeImageQueueUpdate(queue, sources.toString());
        cancelObsolete();
    }
    void track(Request request, Runnable cancel) { if (current(request)) cancellations.put(request, cancel); else cancel.run(); }
    private void cancelObsolete() {
        java.util.Iterator<java.util.Map.Entry<Request, Runnable>> iterator = cancellations.entrySet().iterator();
        while (iterator.hasNext()) {
            java.util.Map.Entry<Request, Runnable> entry = iterator.next();
            if (current(entry.getKey())) continue;
            iterator.remove(); entry.getValue().run();
        }
    }
    boolean current(Request request) {
        return queue != 0 && GpuView.nativeImageQueueCheck(queue, request.token, false);
    }
    List<Request> start() {
        List<Request> requests = new ArrayList<>();
        if (queue == 0) return requests;
        try {
            JSONArray batch = new JSONArray(GpuView.nativeImageQueueStart(queue));
            for (int i = 0; i < batch.length(); i++) {
                JSONObject item = batch.getJSONObject(i);
                requests.add(new Request(item.getString("source"), item.getString("retry_key"), item.getLong("token")));
            }
        } catch (JSONException error) {
            throw new IllegalStateException("Invalid native image request batch", error);
        }
        return requests;
    }
    boolean complete(Request request) {
        cancellations.remove(request);
        return queue != 0 && GpuView.nativeImageQueueCheck(queue, request.token, true);
    }
    boolean apply(long page, Request request, byte[] bytes) {
        long owner = queue;
        return owner != 0 && GpuView.nativeImage(page, owner, request.token, bytes);
    }
    void status(long page, Request request, boolean success) {
        long owner = queue;
        if (owner != 0) GpuView.nativeImageStatus(page, owner, request.token, request.retryKey, success);
    }
    @Override public void close() {
        closed = true;
        if (queue != 0) { GpuView.nativeImageQueueReset(queue, true); queue = 0; }
        cancelObsolete();
    }
}
