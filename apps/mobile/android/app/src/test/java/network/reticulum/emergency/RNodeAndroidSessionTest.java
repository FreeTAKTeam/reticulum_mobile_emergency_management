package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.json.JSONObject;
import org.junit.Test;

public class RNodeAndroidSessionTest {
    @Test
    public void statusTracksOnlyAcceptedInboundAndCompletedOutboundChunks() throws Exception {
        final FakeSession session = new FakeSession(42L);

        session.offerBytes(new byte[] { 1, 2, 3 });
        session.recordWrite(new byte[] { 4, 5 });

        final JSONObject status = new JSONObject(session.statusJson());
        assertEquals(42L, status.getLong("generation"));
        assertEquals("test", status.getString("kind"));
        assertFalse(status.getBoolean("closed"));
        assertEquals(1L, status.getLong("inboundChunks"));
        assertEquals(3L, status.getLong("inboundBytes"));
        assertEquals(1L, status.getLong("outboundChunks"));
        assertEquals(2L, status.getLong("outboundBytes"));
    }

    @Test
    public void failedSessionRetainsTheFirstActionableError() throws Exception {
        final FakeSession session = new FakeSession(7L);

        session.fail("first failure");
        session.fail("later failure");

        final JSONObject status = new JSONObject(session.statusJson());
        assertEquals("first failure", status.getString("lastError"));
    }

    private static final class FakeSession extends RNodeAndroidSession {
        FakeSession(long generation) {
            super(generation);
        }

        @Override
        void open(long timeoutMs) {}

        @Override
        String mode() {
            return "test";
        }

        @Override
        Integer negotiatedMtu() {
            return 517;
        }

        @Override
        void write(byte[] payload, long timeoutMs) {
            recordWrite(payload);
        }

        @Override
        public void close() {
            closed.set(true);
        }
    }
}
