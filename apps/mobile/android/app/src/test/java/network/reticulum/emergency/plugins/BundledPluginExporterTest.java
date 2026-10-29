package network.reticulum.emergency.plugins;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertThrows;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import org.junit.Test;

public class BundledPluginExporterTest {
    private static final String EMPTY_SHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    private static final String HELLO_SHA = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    @Test
    public void requiresExactlyOneOfEachPlugin() throws Exception {
        BundledPluginExporter.validateAssets(new String[] {
            "ble-heart-rate-" + EMPTY_SHA + ".apk", "watch-status-" + EMPTY_SHA + ".apk",
            "atn-rangefinder-" + EMPTY_SHA + ".apk"
        });
        assertThrows(IOException.class, () -> BundledPluginExporter.validateAssets(new String[] {
            "ble-heart-rate-" + EMPTY_SHA + ".apk", "watch-status-" + EMPTY_SHA + ".apk",
            "watch-status-" + EMPTY_SHA + ".apk"
        }));
    }

    @Test
    public void checksBytesBeforePublishingTheFile() throws Exception {
        byte[] data = "hello".getBytes(StandardCharsets.UTF_8);
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        BundledPluginExporter.copyVerified(new ByteArrayInputStream(data), output, "watch-status-" + HELLO_SHA + ".apk");
        assertArrayEquals(data, output.toByteArray());
        assertThrows(IOException.class, () -> BundledPluginExporter.copyVerified(
            new ByteArrayInputStream(data), new ByteArrayOutputStream(), "watch-status-" + EMPTY_SHA + ".apk"));
    }
}
