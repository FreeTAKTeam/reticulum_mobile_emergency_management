package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

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

    @Test
    public void targetedEventTestControlUsesANativeBridgeBoundary() throws Exception {
        final Method method = ReticulumBridge.class.getDeclaredMethod(
            "upsertEventToDestinationJson",
            String.class,
            String.class
        );
        assertTrue(Modifier.isNative(method.getModifiers()));
    }

    private void assertInherited(String name, Class<?>... parameterTypes) throws Exception {
        final Method method = ReticulumNodeService.class.getMethod(name, parameterTypes);
        assertEquals(ReticulumBridgeServiceApi.class, method.getDeclaringClass());
    }
}
