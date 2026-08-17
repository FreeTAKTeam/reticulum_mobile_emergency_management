package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;

import org.junit.Test;

import java.util.Arrays;

public class ReticulumNodePluginBaseTest {
    @Test
    public void concretePluginInheritsSharedLifecycle() throws Exception {
        assertEquals(
            ReticulumNodeChecklistPluginApi.class,
            ReticulumNodePlugin.class.getSuperclass()
        );
        assertEquals(
            ReticulumNodeAppDataPluginApi.class,
            ReticulumNodeChecklistPluginApi.class.getSuperclass()
        );
        assertEquals(
            ReticulumNodeTransportPluginApi.class,
            ReticulumNodeAppDataPluginApi.class.getSuperclass()
        );
        assertEquals(
            ReticulumNodePluginBase.class,
            ReticulumNodeTransportPluginApi.class.getSuperclass()
        );
        assertEquals(
            ReticulumNodePluginBase.class,
            ReticulumNodePlugin.class.getMethod("load").getDeclaringClass()
        );
        assertEquals(
            ReticulumNodePluginBase.class,
            ReticulumNodePluginBase.class.getDeclaredMethod("handleOnResume").getDeclaringClass()
        );
        assertEquals(
            ReticulumNodePluginBase.class,
            ReticulumNodePluginBase.class.getDeclaredMethod("handleOnDestroy").getDeclaringClass()
        );
        assertEquals(
            ReticulumNodeAppDataPluginApi.class,
            ReticulumNodePlugin.class.getMethod("refreshPlugins", PluginCall.class).getDeclaringClass()
        );
        assertEquals(
            ReticulumNodeChecklistPluginApi.class,
            ReticulumNodePlugin.class.getMethod("getChecklists", PluginCall.class).getDeclaringClass()
        );
        assertEquals(
            ReticulumNodePlugin.class,
            ReticulumNodePlugin.class.getMethod("getEams", PluginCall.class).getDeclaringClass()
        );
    }

    @Test
    public void capacitorDiscoversEveryPublicPluginMethod() {
        final long methodCount = Arrays.stream(ReticulumNodePlugin.class.getMethods())
            .filter(method -> method.isAnnotationPresent(PluginMethod.class))
            .count();

        assertEquals(97L, methodCount);
    }
}
