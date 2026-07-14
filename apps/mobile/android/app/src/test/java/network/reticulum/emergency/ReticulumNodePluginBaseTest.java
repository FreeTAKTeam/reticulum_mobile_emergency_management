package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import com.getcapacitor.PluginMethod;

import org.junit.Test;

import java.util.Arrays;

public class ReticulumNodePluginBaseTest {
    @Test
    public void concretePluginInheritsSharedLifecycle() throws Exception {
        assertEquals(
            ReticulumNodeTransportPluginApi.class,
            ReticulumNodePlugin.class.getSuperclass()
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
    }

    @Test
    public void capacitorDiscoversEveryPublicPluginMethod() {
        final long methodCount = Arrays.stream(ReticulumNodePlugin.class.getMethods())
            .filter(method -> method.isAnnotationPresent(PluginMethod.class))
            .count();

        assertEquals(96L, methodCount);
    }
}
