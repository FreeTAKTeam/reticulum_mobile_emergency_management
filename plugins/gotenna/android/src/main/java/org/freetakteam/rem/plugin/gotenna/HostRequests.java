package org.freetakteam.rem.plugin.gotenna;

import java.io.IOException;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONObject;

/** Only the radio worker blocks; Binder callbacks remain free to deliver responses. */
final class HostRequests {
    static final class Rejected extends IOException {
        final String code;
        Rejected(JSONObject error) {
            super(error == null ? "REM request failed" : error.toString());
            code = error == null ? "HostError" : error.optString("code", "HostError");
        }
        boolean isTerminal() { return "InvalidRequest".equals(code) || "RecipientNotLocal".equals(code); }
    }

    private IRemPluginHost host;
    private String requestId;
    private CompletableFuture<JSONObject> pending;

    synchronized void start(IRemPluginHost next) { stop(); host = next; }

    synchronized void stop() {
        host = null;
        if (pending != null) pending.completeExceptionally(new IOException("REM host stopped"));
        pending = null;
        requestId = null;
    }

    JSONObject request(String operation, JSONObject payload) throws Exception {
        final CompletableFuture<JSONObject> result;
        synchronized (this) {
            if (host == null || pending != null) throw new IOException("REM host unavailable or busy");
            requestId = UUID.randomUUID().toString();
            result = new CompletableFuture<>();
            pending = result;
            try { host.submitRequest(PluginProtocol.request(requestId, operation, payload)); }
            catch (Exception error) { pending = null; requestId = null; throw error; }
        }
        try { return result.get(5, TimeUnit.SECONDS); }
        catch (java.util.concurrent.ExecutionException failure) {
            if (failure.getCause() instanceof Exception) throw (Exception) failure.getCause();
            throw failure;
        }
        finally {
            synchronized (this) {
                if (pending == result) { pending = null; requestId = null; }
            }
        }
    }

    synchronized void accept(String raw) {
        try {
            final JSONObject response = PluginProtocol.requireEnvelope(raw);
            if (pending == null || !response.getString("requestId").equals(requestId)) return;
            if (response.optBoolean("ok")) pending.complete(response.getJSONObject("result"));
            else pending.completeExceptionally(new Rejected(response.optJSONObject("error")));
        } catch (Exception error) {
            if (pending != null) pending.completeExceptionally(error);
        }
    }
}
