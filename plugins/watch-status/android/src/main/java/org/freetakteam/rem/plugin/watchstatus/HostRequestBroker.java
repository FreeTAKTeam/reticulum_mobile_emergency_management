package org.freetakteam.rem.plugin.watchstatus;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;
import network.reticulum.emergency.plugin.api.PluginProtocol;

/** Correlates asynchronous Binder replies without blocking a Binder callback thread. */
final class HostRequestBroker implements AutoCloseable {
    interface Sender { void submit(String json) throws Exception; }
    private static final int MAX_PENDING = 4;
    private final Map<String, Pending> pending = new HashMap<>();
    private final ScheduledExecutorService deadlines = Executors.newSingleThreadScheduledExecutor(r -> {
        final Thread thread = new Thread(r, "rem-watch-host-timeouts");
        thread.setDaemon(true);
        return thread;
    });
    private final long timeoutMs;
    private Sender sender;

    private static final class Pending {
        final CompletableFuture<JSONObject> result = new CompletableFuture<>();
        ScheduledFuture<?> timeout;
    }

    HostRequestBroker() { this(1_500); }
    HostRequestBroker(long timeoutMs) { this.timeoutMs = timeoutMs; }

    synchronized void start(Sender next) {
        stop();
        sender = next;
    }

    synchronized void stop() {
        sender = null;
        final Pending[] cancelled = pending.values().toArray(new Pending[0]);
        pending.clear();
        for (Pending item : cancelled) {
            if (item.timeout != null) item.timeout.cancel(false);
            item.result.completeExceptionally(new RadarFailure("HostUnavailable", 503));
        }
    }

    synchronized CompletableFuture<JSONObject> request(String operation, JSONObject payload) {
        final Pending item = new Pending();
        if (sender == null || pending.size() >= MAX_PENDING) {
            item.result.completeExceptionally(new RadarFailure(sender == null ? "HostUnavailable" : "Busy", 503));
            return item.result;
        }
        final String id = UUID.randomUUID().toString();
        try {
            final String json = PluginProtocol.request(id, operation, payload);
            pending.put(id, item);
            item.timeout = deadlines.schedule(() -> fail(id, new RadarFailure("HostTimeout", 503)), timeoutMs, TimeUnit.MILLISECONDS);
            sender.submit(json);
        } catch (Exception error) {
            final RadarFailure failure = error instanceof RadarFailure ? (RadarFailure) error : new RadarFailure("HostUnavailable", 503);
            fail(id, failure);
            item.result.completeExceptionally(failure);
        }
        return item.result;
    }

    void accept(String json) {
        try {
            final JSONObject response = PluginProtocol.requireEnvelope(json);
            final Pending item = take(response.optString("requestId", ""));
            if (item == null) return;
            if (response.optBoolean("ok") && response.optJSONObject("result") != null) {
                item.result.complete(response.getJSONObject("result"));
            } else {
                final JSONObject error = response.optJSONObject("error");
                final String code = error == null ? "HostError" : error.optString("code", "HostError");
                final String safeCode = java.util.Arrays.asList("PermissionDenied", "InvalidRequest", "NotFound", "ResponseTooLarge").contains(code) ? code : "HostError";
                item.result.completeExceptionally(new RadarFailure(safeCode, "PermissionDenied".equals(safeCode) ? 403 : 503));
            }
        } catch (Exception invalid) {
            // Uncorrelatable replies expire on the original fixed deadline.
        }
    }

    private synchronized Pending take(String id) {
        final Pending item = pending.remove(id);
        if (item != null && item.timeout != null) item.timeout.cancel(false);
        return item;
    }

    private void fail(String id, RadarFailure error) {
        final Pending item = take(id);
        if (item != null) item.result.completeExceptionally(error);
    }

    synchronized int pendingCount() { return pending.size(); }

    @Override public void close() {
        stop();
        deadlines.shutdownNow();
    }
}
