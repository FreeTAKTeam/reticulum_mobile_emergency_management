package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

public class RuntimeConfigResolverTest {
    @Test
    public void canonicalizeSortsObjectKeysAndPreservesArrayOrder() throws Exception {
        final JSONObject nested = new JSONObject()
            .put("z", true)
            .put("a", JSONObject.NULL);
        final JSONObject config = new JSONObject()
            .put("storageDir", "relative")
            .put("interfaces", new JSONArray().put("tcp").put("rnode"))
            .put("nested", nested);

        assertEquals(
            "{\"interfaces\":[\"tcp\",\"rnode\"],\"nested\":{\"a\":null,\"z\":true},\"storageDir\":\"relative\"}",
            RuntimeConfigResolver.canonicalize(config)
        );
    }

    @Test
    public void canonicalizeEscapesStringsAndKeepsScalarTypes() throws Exception {
        final JSONArray values = new JSONArray()
            .put("line\nbreak")
            .put(42)
            .put(false);

        assertEquals(
            "[\"line\\nbreak\",42,false]",
            RuntimeConfigResolver.canonicalize(values)
        );
    }

    @Test
    public void bluetoothIdsIgnoreCommonAddressSeparatorsAndCase() {
        assertEquals(
            "48ca4338bce1",
            RuntimeConfigResolver.normalizeBluetoothId(" 48:CA:43:38:BC:E1 ")
        );
        assertEquals(
            "48ca4338bce1",
            RuntimeConfigResolver.normalizeBluetoothId("48-ca-43-38-bc-e1")
        );
        assertEquals("", RuntimeConfigResolver.normalizeBluetoothId(null));
    }
}
