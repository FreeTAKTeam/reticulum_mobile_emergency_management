package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import java.lang.reflect.Method;

import org.junit.Test;

public class ReticulumBridgeServiceApiTest {
    @Test
    public void concreteServicePreservesInheritedBridgeMethods() throws Exception {
        assertInherited("connectPeer", String.class);
        assertInherited("sendLxmfJson", String.class);
        assertInherited("listMessagesJson", String.class);
        assertInherited("getChecklistsJson", String.class);
        assertInherited("upsertEamJson", String.class);
        assertInherited("upsertEventJson", String.class);
        assertInherited("recordLocalTelemetryFixJson", String.class);
        assertInherited("listSosAlertsJson");
        assertInherited("setAnnounceCapabilities", String.class);
        assertInherited("refreshHubDirectory");
        assertInherited("getHubDirectorySnapshotJson");
        assertInherited("setActiveTeamJson", String.class);
    }

    private void assertInherited(String name, Class<?>... parameterTypes) throws Exception {
        final Method method = ReticulumNodeService.class.getMethod(name, parameterTypes);
        assertEquals(ReticulumBridgeServiceApi.class, method.getDeclaringClass());
    }
}
