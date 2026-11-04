package org.freetakteam.rem.plugin.gotenna;

import static org.junit.Assert.*;
import java.util.Arrays;
import java.util.List;
import org.json.JSONObject;
import org.junit.Test;

public final class MeshProtocolTest {
    private static byte[] unhex(String hex) {
        final byte[] data = new byte[hex.length() / 2];
        for (int i = 0; i < data.length; i++) data[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
        return data;
    }

    @Test public void matchesExistingRustLocationAndWireVectors() throws Exception {
        final byte[] payload = MeshMessage.shout(0xc0dec48459bfL, 1_760_000_000L, 7, "UBUNTU-MESH-TEST",
            "compatible 🛰️", 43.838654, -66.115223, "Yarmouth, NS");
        assertEquals("0603023fff056afb10000000c0dec48459bf68e7780000070001013103105542554e54552d4d4553482d544553540412636f6d70617469626c6520f09f9bb0efb88f062b070c5961726d6f7574682c204e5308084045eb5903a7546d0908c050875fd04a2fcf0a0133130468e77800bf33220101", ReceiveArchive.hex(payload));
        final JSONObject message = MeshMessage.decode(payload);
        assertEquals("c0dec48459bf", message.getString("senderGid"));
        assertEquals(43.838654, message.getDouble("lat"), 0);
        assertEquals(-66.115223, message.getDouble("lon"), 0);
        assertEquals("compatible 🛰️", message.getString("text"));
        assertEquals("100203040603023fff056afb1010000000c0dec48459bf68e778000007000101310310105542554e54552d4d4553482d544553540412636f6d70617469626c6520f09f9bb0efb88f062b070c5961726d6f7574682c204e5308084045eb5903a7546d0908c050875fd04a2fcf0a0133130468e77800bf33220101f16b1003", ReceiveArchive.hex(MeshFraming.encode(3, 4, payload)));
    }

    @Test public void reassemblesAtEveryBoundaryIncludingEscapedDle() {
        final byte[] payload = new byte[] {0x10, 0x02, 0x03, 0x10};
        final byte[] wire = MeshFraming.encode(6, 17, payload);
        for (int cut = 0; cut <= wire.length; cut++) {
            final MeshFraming decoder = new MeshFraming();
            final java.util.ArrayList<byte[]> frames = new java.util.ArrayList<>();
            frames.addAll(decoder.receive(Arrays.copyOfRange(wire, 0, cut)));
            frames.addAll(decoder.receive(Arrays.copyOfRange(wire, cut, wire.length)));
            assertEquals(1, frames.size());
            assertArrayEquals(MeshMessage.join(new byte[] {6, 17}, payload), frames.get(0));
        }
    }

    @Test public void rejectsBadChecksumsTruncationDuplicatesAndInvalidCoordinates() {
        final byte[] wire = MeshFraming.encode(4, 1, new byte[0]);
        wire[3] ^= 1;
        assertThrows(IllegalArgumentException.class, () -> new MeshFraming().receive(wire));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.fields(new byte[] {1, 3, 1}));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.fields(new byte[] {1, 0, 1, 0}));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.coordinates(Double.NaN, 0));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.coordinates(0, 181));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.gid("000000000000"));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.shout(1, 1, 1, "REM", "🛰️".repeat(40), null, null, ""));
        assertThrows(IllegalArgumentException.class, () -> MeshMessage.decode(unhex("0603023fff0501ff")));
    }

    @Test public void chatCarriesItsRecipientAndReassemblesUnicodeOutOfOrder() throws Exception {
        final String text = "Message from another REM interface 🛰️ ".repeat(9);
        final JSONObject item = new JSONObject().put("id", "a".repeat(64)).put("source", "b".repeat(32))
            .put("kind", "text").put("payload", new JSONObject().put("messageId", "c".repeat(64))
                .put("destinationHex", "d".repeat(32)).put("text", text));
        final List<byte[]> parts = MeshRelay.encode(item, 1, "Gateway", 8, 1_760_000_000_000L);
        assertTrue(parts.size() > 1);
        final MeshRoutedFrames receiver = new MeshRoutedFrames();
        JSONObject complete = null;
        for (int i = parts.size() - 1; i >= 0; i--) {
            final JSONObject decoded = MeshMessage.decode(parts.get(i));
            assertTrue(MeshMessage.utf8(decoded.getString("text")).length <= 180);
            assertTrue(decoded.getString("text").contains("d".repeat(32)));
            complete = receiver.receive(decoded, 1000);
        }
        assertNotNull(complete);
        assertEquals(text, complete.getString("text"));
        assertEquals("d".repeat(32), complete.getString("destinationHex"));
        assertEquals("b".repeat(32), complete.getString("sourceHex"));
    }

    @Test public void rejectsUnaddressedTextAndConflictingFragments() throws Exception {
        final MeshRoutedFrames receiver = new MeshRoutedFrames();
        assertThrows(IllegalArgumentException.class, () -> receiver.receive(new JSONObject().put("senderGid", "001122334455").put("text", "hello everyone"), 0));
        final String header = "REM1 C " + "a".repeat(32) + " " + "b".repeat(32) + " " + "c".repeat(32) + " 1/2 ";
        assertNull(receiver.receive(new JSONObject().put("senderGid", "001122334455").put("text", header + "first"), 0));
        assertThrows(IllegalArgumentException.class, () -> receiver.receive(new JSONObject().put("senderGid", "001122334455").put("text", header + "changed"), 1));
        assertThrows(IllegalArgumentException.class, () -> MeshRoutedFrames.encode("C", "a".repeat(32), "b".repeat(32), "", "hello"));
    }

    @Test public void partialMessageCustodySurvivesRestartWithoutExtendingExpiry() throws Exception {
        final List<String> parts = MeshRoutedFrames.encode("C", "a".repeat(32), "b".repeat(32), "c".repeat(32), "restart-safe chat ".repeat(15));
        final MeshRoutedFrames first = new MeshRoutedFrames();
        assertNull(first.receive(new JSONObject().put("senderGid", "001122334455").put("text", parts.get(0)), 1000));
        final String checkpoint = first.snapshot();
        final MeshRoutedFrames restarted = new MeshRoutedFrames();
        restarted.restore(checkpoint, 2000);
        JSONObject complete = null;
        for (int i = 1; i < parts.size(); i++) complete = restarted.receive(new JSONObject().put("senderGid", "001122334455").put("text", parts.get(i)), 2000);
        assertNotNull(complete);
        assertEquals("restart-safe chat ".repeat(15), complete.getString("text"));
        final MeshRoutedFrames expired = new MeshRoutedFrames();
        expired.restore(checkpoint, 122000);
        assertEquals("{}", expired.snapshot());
    }

    @Test public void eventFramesCarryTheActualMecpCode() throws Exception {
        final JSONObject item = new JSONObject().put("id", "a".repeat(64)).put("source", "b".repeat(32)).put("kind", "event")
            .put("payload", new JSONObject().put("code", "MECP/2/P01"));
        final JSONObject decoded = MeshMessage.decode(MeshRelay.encode(item, 1, "Gateway", 1, 100000).get(0));
        assertTrue(decoded.getString("text").endsWith(" MECP/2/P01"));
        final JSONObject received = new MeshRoutedFrames().receive(decoded, 0);
        assertEquals("event", received.getString("kind"));
        assertEquals("MECP/2/P01", received.getString("text"));
        assertEquals("-", received.getString("destinationHex"));
    }

    @Test public void preservesRemotePositionWithoutImpersonatingItsSender() throws Exception {
        final JSONObject item = new JSONObject().put("id", "c".repeat(64)).put("source", "d".repeat(32)).put("kind", "position")
            .put("payload", new JSONObject().put("callsign", "Remote unit").put("lat", 45).put("lon", -63));
        final JSONObject received = MeshMessage.decode(MeshRelay.encode(item, 123, "G".repeat(32), 1, 1_760_000_000_000L).get(0));
        assertEquals("00000000007b", received.getString("senderGid"));
        assertEquals(45, received.getDouble("lat"), 0);
        assertEquals(-63, received.getDouble("lon"), 0);
        assertTrue(received.getString("label").contains("Remote unit"));
        assertTrue(MeshRelay.isEcho(received));
    }

    @Test public void oldQueuedPositionsCannotMoveNewerMarkersBackward() throws Exception {
        final JSONObject previous = new JSONObject().put("timestamp", 100).put("sequence", 65535);
        assertFalse(MeshPositionOrder.isNewer(new JSONObject().put("timestamp", 99).put("sequence", 2), previous));
        assertFalse(MeshPositionOrder.isNewer(previous, previous));
        assertTrue(MeshPositionOrder.isNewer(new JSONObject().put("timestamp", 100).put("sequence", 0), previous));
        assertTrue(MeshPositionOrder.isNewer(new JSONObject().put("timestamp", 101).put("sequence", 1), previous));
    }

    @Test public void consumesActualNativePollFixtureWhenProvided() throws Exception {
        final String path = System.getenv("GOTENNA_RELAY_FIXTURE");
        org.junit.Assume.assumeTrue(path != null && !path.isEmpty());
        final JSONObject poll = new JSONObject(new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(path)), java.nio.charset.StandardCharsets.UTF_8));
        final org.json.JSONArray items = poll.getJSONArray("items");
        assertEquals(2, items.length());
        for (int i = 0; i < items.length(); i++) {
            final JSONObject item = items.getJSONObject(i);
            for (byte[] payload : MeshRelay.encode(item, 0xc0dec48459bfL, "REM", 7, System.currentTimeMillis())) {
                final byte[] wire = MeshFraming.encode(3, 4, payload);
                final byte[] frame = new MeshFraming().receive(wire).get(0);
                final JSONObject decoded = MeshMessage.decode(Arrays.copyOfRange(frame, 2, frame.length));
                if ("position".equals(item.getString("kind"))) assertEquals(item.getJSONObject("payload").getDouble("lat"), decoded.getDouble("lat"), 0);
                else {
                    final JSONObject routed = new MeshRoutedFrames().receive(decoded, 0);
                    assertEquals("native ingress", routed.getString("text"));
                    assertEquals(item.getJSONObject("payload").getString("destinationHex"), routed.getString("destinationHex"));
                }
            }
        }
    }
}
