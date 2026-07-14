package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.json.JSONObject;
import org.junit.Test;

public class OperationalNotificationStateTest {
    @Test
    public void deduplicatesEachOperationalKeyType() {
        final OperationalNotificationState state = new OperationalNotificationState();

        assertTrue(state.markEam("eam:1"));
        assertFalse(state.markEam("eam:1"));
        assertTrue(state.markEvent("event:1"));
        assertFalse(state.markEvent("event:1"));
        assertTrue(state.markChecklist("checklist:1"));
        assertFalse(state.markChecklist("checklist:1"));
        assertTrue(state.markMessage("message:1"));
        assertFalse(state.markMessage("message:1"));
        assertTrue(state.markMissionPacket("packet:1"));
        assertFalse(state.markMissionPacket("packet:1"));
    }

    @Test
    public void checklistKeyUsesTheLatestCompatibleTimestamp() throws Exception {
        final OperationalNotificationState state = new OperationalNotificationState();
        final JSONObject checklist = new JSONObject()
            .put("uid", "ABC123")
            .put("updated_at", "2026-07-14T12:00:00Z")
            .put("uploadedAt", "2026-07-14T12:30:00Z");

        assertEquals("abc123:2026-07-14T12:30:00Z", state.checklistKey(checklist));
        assertEquals("", state.checklistKey(new JSONObject().put("uid", "ABC123")));
    }

    @Test
    public void readsCamelAndSnakeCaseProjectionFields() throws Exception {
        final OperationalNotificationState state = new OperationalNotificationState();
        final JSONObject snakeCase = new JSONObject()
            .put("pending_count", 4)
            .put("changed_by", "peer-a");

        assertEquals(4, state.optIntAny(snakeCase, "pendingCount", "pending_count", 0));
        assertEquals("peer-a", state.optStringAny(snakeCase, "changedBy", "changed_by"));
        assertEquals(7, state.optIntAny(null, "pendingCount", "pending_count", 7));
        assertEquals("", state.optStringAny(null, "changedBy", "changed_by"));
    }
}
