package org.freetakteam.rem.plugin.gotenna;

import static org.junit.Assert.*;
import android.os.IBinder;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import org.json.JSONObject;
import org.junit.Test;

public final class HostRequestsTest {
    @Test public void preservesTerminalVersusRetriableNativeRejections() throws Exception {
        for (String code : new String[] {"InvalidRequest", "RecipientNotLocal", "PermissionDenied", "SubmissionUnconfirmed"}) {
            final HostRequests requests = new HostRequests();
            requests.start(new IRemPluginHost() {
                @Override public IBinder asBinder() { return null; }
                @Override public void submitRequest(String raw) {
                    try {
                        final String id = new JSONObject(raw).getString("requestId");
                        requests.accept(new JSONObject().put("protocolVersion", 1).put("requestId", id).put("ok", false)
                            .put("error", new JSONObject().put("code", code).put("message", "fixture")).toString());
                    } catch (Exception failure) { throw new AssertionError(failure); }
                }
            });
            final HostRequests.Rejected rejected = assertThrows(HostRequests.Rejected.class,
                () -> requests.request("relay.chat.accept", new JSONObject()));
            assertEquals(code, rejected.code);
            assertEquals("InvalidRequest".equals(code) || "RecipientNotLocal".equals(code), rejected.isTerminal());
            requests.stop();
        }
    }
}
