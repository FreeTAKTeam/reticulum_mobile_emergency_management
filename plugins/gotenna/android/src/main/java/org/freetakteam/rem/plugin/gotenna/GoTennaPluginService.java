package org.freetakteam.rem.plugin.gotenna;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothProfile;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.os.Build;
import android.util.Log;
import androidx.core.app.NotificationCompat;
import java.io.IOException;
import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import network.reticulum.emergency.plugin.api.RemPluginService;
import org.json.JSONArray;
import org.json.JSONObject;

public final class GoTennaPluginService extends RemPluginService {
    private static final String CHANNEL = "rem-gotenna";
    private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor();
    private final HostRequests requests = new HostRequests();
    private final AtomicBoolean busy = new AtomicBoolean();
    private final AtomicLong generation = new AtomicLong();
    private final ArrayDeque<RelayPart> relay = new ArrayDeque<>();
    private SharedPreferences preferences;
    private ReceiveArchive archive;
    private final MeshRoutedFrames routedFrames = new MeshRoutedFrames();
    private String localSource = "";
    private final java.util.Map<String, String> blockedDeliveries = new java.util.HashMap<>();
    private volatile MeshBleSession session;
    private volatile boolean running;
    private volatile String status = "Start the REM node to connect";
    private volatile String error = "";
    private Long cursor;
    private long relayGeneration;

    private static final class RelayPart {
        final String id;
        final byte[] payload;
        final long receivedAtMs;
        RelayPart(String id, byte[] payload, long receivedAtMs) {
            this.id = id; this.payload = payload; this.receivedAtMs = receivedAtMs;
        }
    }

    @Override public void onCreate() {
        super.onCreate();
        preferences = getSharedPreferences("gotenna", MODE_PRIVATE);
        archive = new ReceiveArchive(getFilesDir(), preferences);
        getSystemService(NotificationManager.class).createNotificationChannel(
            new NotificationChannel(CHANNEL, "REM goTenna connection", NotificationManager.IMPORTANCE_LOW));
        worker.scheduleWithFixedDelay(() -> {
            if (!running || session == null || !busy.compareAndSet(false, true)) return;
            final long current = generation.get();
            try {
                final MeshBleSession radio = session;
                if (radio == null || !radio.isReady()) throw new IOException(radio == null ? "Mesh disconnected" : radio.failureMessage());
                receiveNext(current);
                if (generation.get() == current && running) relayNext(current);
            } catch (Exception failure) { report(failure); }
            finally { busy.set(false); }
        }, 5, 5, TimeUnit.SECONDS);
    }

    @Override protected Set<String> allowedHostPackageNames() { return Collections.singleton(BuildConfig.REM_HOST_PACKAGE); }
    @Override protected Set<String> allowedHostCertificateFingerprints() {
        if (BuildConfig.REM_HOST_FINGERPRINTS.trim().isEmpty()) return Collections.emptySet();
        return new HashSet<>(Arrays.asList(BuildConfig.REM_HOST_FINGERPRINTS.toLowerCase(Locale.ROOT).split(",")));
    }
    @Override protected String getDescriptorJson() {
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.gotenna\",\"apiMajor\":1,\"apiMinor\":5}";
    }
    @Override protected synchronized void onPluginStart(IRemPluginHost host, String raw) {
        onPluginStop("restarted");
        try {
            if (new JSONObject(raw).optInt("apiMinor") < 5) throw new IOException("REM plugin API 1.5 required");
            requests.start(host);
            running = true;
            status = "Disconnected";
        } catch (Exception failure) { report(failure); }
    }
    @Override protected synchronized void onPluginStop(String reason) {
        running = false;
        generation.incrementAndGet();
        requests.stop();
        disconnect();
        status = "REM node stopped";
    }
    @Override protected void onHostEvent(String raw) {}
    @Override protected void onHostResponse(String raw) { requests.accept(raw); }
    @Override public boolean onUnbind(Intent intent) { onPluginStop("unbound"); return super.onUnbind(intent); }
    @Override public void onDestroy() { onPluginStop("destroyed"); worker.shutdownNow(); super.onDestroy(); }

    @Override protected void onConfigurationRequest(String raw, IRemPluginConfigurationCallback callback) {
        try {
            PluginProtocol.requireJsonSize(raw, "goTenna configuration");
            final JSONObject request = new JSONObject(raw);
            final String action = request.optString("action");
            if ("update".equals(request.optString("type"))) update(request);
            if ("action".equals(request.optString("type"))) {
                if ("pair".equals(action)) {
                    respond(callback, new JSONObject().put("type", "actionResult").put("activity",
                        new JSONObject().put("className", PairingActivity.class.getName())));
                    return;
                }
                if ("disconnect".equals(action)) {
                    generation.incrementAndGet();
                    disconnect();
                    status = "Disconnected";
                } else if (!"pair".equals(action)) {
                    if (!running) throw new IOException("Start the REM node first");
                    if (!busy.compareAndSet(false, true)) throw new IOException("Radio action in progress");
                    final long current = generation.get();
                    worker.execute(() -> {
                        try {
                            requireCurrent(current);
                            if ("connect".equals(action)) connect(current);
                            else if ("sendText".equals(action) || "sendPosition".equals(action)) send(request, current);
                            else throw new IOException("Unknown goTenna action");
                        } catch (Exception failure) { report(failure); }
                        finally { busy.set(false); }
                    });
                }
            }
            respond(callback, state());
        } catch (Exception failure) {
            try { respond(callback, new JSONObject().put("type", "validationError").put("message", failure.getMessage())); }
            catch (Exception delivery) { Log.w("REM.goTenna", "Configuration response failed", delivery); }
        }
    }

    private void respond(IRemPluginConfigurationCallback callback, JSONObject response) throws Exception {
        PluginProtocol.requireJsonSize(response.toString(), "goTenna state");
        callback.onResponse(response.toString());
    }

    private void update(JSONObject values) throws Exception {
        final String gid = values.getString("gid").trim();
        final String nickname = values.getString("nickname").trim();
        if (!gid.isEmpty()) MeshMessage.gid(gid);
        if (MeshMessage.utf8(nickname).length < 1 || MeshMessage.utf8(nickname).length > 32) throw new IOException("Nickname requires 1–32 UTF-8 bytes");
        if (values.optBoolean("relayEnabled") && gid.isEmpty()) throw new IOException("Configure your Mesh GID before enabling public relay");
        if (!preferences.edit().putString("gid", gid).putString("nickname", nickname)
            .putBoolean("relayEnabled", values.getBoolean("relayEnabled")).putLong("relayVersion", System.nanoTime()).commit()) {
            throw new IOException("Configuration could not be saved");
        }
    }

    private JSONObject state() throws Exception {
        return new JSONObject().put("type", "state").put("status", status).put("error", error)
            .put("busy", busy.get()).put("connected", session != null && session.isReady())
            .put("deviceName", preferences.getString("deviceName", "Not paired"))
            .put("address", preferences.getString("address", "")).put("gid", preferences.getString("gid", ""))
            .put("nickname", preferences.getString("nickname", "REM"))
            .put("relayEnabled", preferences.getBoolean("relayEnabled", false));
    }

    @SuppressLint("MissingPermission")
    private void connect(long current) throws Exception {
        disconnect();
        if (Build.VERSION.SDK_INT >= 31 && checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED) {
            throw new IOException("Open Pair radio and grant Bluetooth permission");
        }
        final BluetoothManager manager = getSystemService(BluetoothManager.class);
        if (manager == null || manager.getAdapter() == null || !manager.getAdapter().isEnabled()) throw new IOException("Enable Bluetooth first");
        final String address = preferences.getString("address", "");
        if (address.isEmpty()) throw new IOException("Pair a Mesh radio first");
        for (android.bluetooth.BluetoothDevice device : manager.getConnectedDevices(BluetoothProfile.GATT)) {
            if (address.equalsIgnoreCase(device.getAddress())) throw new IOException("Radio already connected to another app; disconnect it there first");
        }
        startForeground(18014, new NotificationCompat.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
            .setContentTitle("REM goTenna Mesh").setContentText("Receiving Mesh messages and positions").setOngoing(true).build());
        final MeshBleSession next = new MeshBleSession();
        session = next;
        try {
            status = "Connecting";
            next.connect(this, manager.getAdapter().getRemoteDevice(address));
            localSource = requests.request("relay.identity", new JSONObject()).getString("destinationHex");
            routedFrames.restore(preferences.getString("routedAssemblies", "{}"), System.currentTimeMillis());
            blockedDeliveries.clear();
            requireCurrent(current);
            cursor = null;
            relay.clear();
            error = "";
            status = "Connected";
        } catch (Exception failure) { next.close(); stopForeground(STOP_FOREGROUND_REMOVE); throw failure; }
    }

    private void disconnect() {
        final MeshBleSession previous = session;
        session = null;
        if (previous != null) previous.close();
        localSource = "";
        stopForeground(STOP_FOREGROUND_REMOVE);
    }

    private void requireCurrent(long current) throws IOException {
        if (!running || generation.get() != current) throw new IOException("Radio action cancelled");
    }

    private int reserveSequences(int count) throws IOException {
        final int sequence = preferences.getInt("messageSequence", 0);
        if (!preferences.edit().putInt("messageSequence", (sequence + count) & 0xffff).commit()) throw new IOException("Could not save Mesh sequence");
        return sequence;
    }

    private void send(JSONObject request, long current) throws Exception {
        final MeshBleSession radio = session;
        if (radio == null || !radio.isReady()) throw new IOException("Connect the radio first");
        final long gid = MeshMessage.gid(preferences.getString("gid", ""));
        Double lat = null;
        Double lon = null;
        if ("sendPosition".equals(request.getString("action"))) {
            final JSONObject fix = requests.request("map.observer.snapshot", new JSONObject());
            if (!fix.optBoolean("available") || fix.optLong("ageMs", Long.MAX_VALUE) > 120_000
                || !"localDevice".equals(fix.getJSONObject("observer").getString("kind"))) throw new IOException("A local position less than two minutes old is required");
            lat = fix.getDouble("lat"); lon = fix.getDouble("lon");
        }
        final String nickname = preferences.getString("nickname", "REM");
        requireCurrent(current);
        if ("sendText".equals(request.getString("action"))) {
            final String destination = request.getString("destinationHex").trim().toLowerCase(Locale.ROOT);
            final String id = java.util.UUID.randomUUID().toString().replace("-", "");
            final String text = request.getString("text");
            final java.util.List<String> parts = MeshRoutedFrames.encode("C", id, localSource, destination, text);
            blockedDeliveries.remove("pendingSent");
            deliverPending(current, "pendingSent");
            if (preferences.contains("pendingSent")) throw new IOException("Finish pending native history admission before sending another chat");
            final JSONObject admission = new JSONObject().put("id", id).put("sourceHex", localSource)
                .put("destinationHex", destination).put("text", text).put("radioGid", String.format(Locale.ROOT, "%012x", gid));
            requests.request("relay.chat.pending", admission);
            admission.put("kind", "sent").put("radioAccepted", false);
            if (!preferences.edit().putString("pendingSent", admission.toString()).commit()) throw new IOException("Could not checkpoint addressed send; no radio bytes sent");
            final int first = reserveSequences(parts.size());
            for (int i = 0; i < parts.size(); i++) {
                requireCurrent(current);
                radio.command(3, MeshMessage.shout(gid, System.currentTimeMillis() / 1000, first + i, nickname, parts.get(i), null, null, ""));
            }
            admission.put("radioAccepted", true);
            if (!preferences.edit().putString("pendingSent", admission.toString()).commit()) throw new IOException("Radio accepted; history receipt remains unconfirmed");
            deliverPending(current, "pendingSent");
            status = "Addressed chat accepted by radio; recipient delivery unconfirmed";
        } else {
            radio.command(3, MeshMessage.shout(gid, System.currentTimeMillis() / 1000, reserveSequences(1), nickname,
                "Position", lat, lon, nickname));
            status = "Position accepted by radio; recipient delivery unconfirmed";
        }
        if (!preferences.contains("pendingSent")) error = "";
    }

    private void receiveNext(long current) throws Exception {
        final MeshBleSession radio = session;
        if (radio == null) return;
        deliverPending(current, "pendingSent");
        deliverPending(current, "pendingRouted");
        if (preferences.contains("pendingRouted")) return;
        final byte[] raw = radio.command(6, new byte[0]);
        if (raw == null) return;
        JSONObject message;
        try { message = MeshMessage.decode(raw); }
        catch (Exception unsupported) { message = new JSONObject().put("unsupported", unsupported.getMessage()); }
        archive.save(raw, message);
        requireCurrent(current);
        if (!message.has("unsupported")) {
            if (message.has("lat") && !MeshRelay.isEcho(message)) {
                try { importPosition(message, current); }
                catch (Exception processing) { processingError("Map import", processing); }
            } else if (!message.has("lat")) {
                try {
                    final JSONObject routed = routedFrames.receive(message, System.currentTimeMillis());
                    final SharedPreferences.Editor editor = preferences.edit().putString("routedAssemblies", routedFrames.snapshot());
                    if (routed != null) editor.putString("pendingRouted", routed.toString());
                    if (!editor.commit()) {
                        try { routedFrames.restore(preferences.getString("routedAssemblies", "{}"), System.currentTimeMillis()); }
                        catch (Exception restore) { throw new IOException("Could not restore previous assembly checkpoint", restore); }
                        throw new IOException("Could not preserve addressed message assembly");
                    }
                    deliverPending(current, "pendingRouted");
                } catch (IOException custody) { throw custody; }
                catch (Exception processing) { processingError("Addressed message/event", processing); }
            }
        }
        requireCurrent(current);
        radio.command(7, new byte[0]);
        status = "Processed Mesh reception";
    }

    private void deliverPending(long current, String key) throws Exception {
        if (!preferences.contains(key)) return;
        final String version = current + ":" + preferences.getLong("relayVersion", 0);
        if (version.equals(blockedDeliveries.get(key))) return;
        final JSONObject routed = new JSONObject(preferences.getString(key, "{}"));
        final String kind = routed.getString("kind");
        final boolean radioAccepted = routed.optBoolean("radioAccepted");
        routed.remove("kind");
        routed.remove("radioAccepted");
        try {
            requireCurrent(current);
            final String operation = "sent".equals(kind) ? (radioAccepted ? "relay.chat.sent" : "relay.chat.pending")
                : "chat".equals(kind) ? "relay.chat.accept" : "relay.event.accept";
            requests.request(operation, routed);
            if (!preferences.edit().remove(key).commit()) throw new IOException("Could not checkpoint native chat/event acceptance");
            blockedDeliveries.remove(key);
        } catch (HostRequests.Rejected rejected) {
            if (rejected.isTerminal()) {
                if (!preferences.edit().remove(key).commit()) throw new IOException("Could not checkpoint rejected message", rejected);
                error = "Rejected invalid addressed payload: " + rejected.getMessage();
            } else blockDelivery(key, version, rejected);
        } catch (Exception failure) { blockDelivery(key, version, failure); }
    }

    private void blockDelivery(String key, String version, Exception failure) {
        blockedDeliveries.put(key, version);
        error = "Native chat/event admission pending: " + failure.getMessage()
            + ". Check grants/routing and save settings or reconnect to retry this ID without retransmitting radio bytes.";
        Log.w("REM.goTenna", "Native message admission pending", failure);
    }

    private void importPosition(JSONObject message, long current) throws Exception {
        final String sender = message.getString("senderGid");
        final JSONObject positions = new JSONObject(preferences.getString("importedPositions", "{}"));
        if (!MeshPositionOrder.isNewer(message, positions.optJSONObject(sender))) return;
        if (!positions.has(sender) && positions.length() >= 256) throw new IOException("Position sender limit reached; bytes remain in the internal receive archive");
        requireCurrent(current);
        final JSONObject symbols = requests.request("map.symbols.list", new JSONObject());
        final JSONArray items = symbols.getJSONArray("items");
        if (items.length() == 0) throw new IOException("REM has no map symbols");
        String symbol = items.getJSONObject(0).getString("id");
        for (int i = 0; i < items.length(); i++) {
            if ("unknown".equals(items.getJSONObject(i).optString("id"))) symbol = "unknown";
        }
        requireCurrent(current);
        final JSONObject created = requests.request("map.markers.create", new JSONObject().put("idempotencyKey", "gotenna-" + sender)
            .put("symbol", symbol).put("name", "goTenna " + sender).put("notes", "[goTenna:" + sender + "] External, unauthenticated position")
            .put("lat", message.getDouble("lat")).put("lon", message.getDouble("lon")));
        final String marker = created.getJSONObject("marker").getString("object_destination_hash");
        if (!created.optBoolean("created")) {
            requireCurrent(current);
            requests.request("map.markers.position.update", new JSONObject().put("objectDestinationHash", marker)
                .put("lat", message.getDouble("lat")).put("lon", message.getDouble("lon")));
        }
        positions.put(sender, new JSONObject().put("timestamp", message.getLong("timestamp")).put("sequence", message.getInt("sequence")));
        if (!preferences.edit().putString("importedPositions", positions.toString()).commit()) throw new IOException("Could not save position ordering");
    }

    private void relayNext(long current) throws Exception {
        final long version = preferences.getLong("relayVersion", 0);
        if (!preferences.getBoolean("relayEnabled", false)) { cursor = null; relay.clear(); return; }
        if (version != relayGeneration) { cursor = null; relay.clear(); relayGeneration = version; }
        if (cursor == null || relay.isEmpty()) {
            final JSONObject poll = new JSONObject().put("limit", 1);
            if (cursor != null) poll.put("cursor", cursor);
            final JSONObject response = pollRelay(poll);
            cursor = response.getLong("cursor");
            if (response.optBoolean("gap")) error = "Relay journal gap; older traffic was skipped";
            final JSONArray items = response.getJSONArray("items");
            for (int i = 0; i < items.length(); i++) {
                final JSONObject item = items.getJSONObject(i);
                final java.util.List<byte[]> parts = MeshRelay.encode(item, MeshMessage.gid(preferences.getString("gid", "")),
                    preferences.getString("nickname", "REM"), reserveSequences(24), System.currentTimeMillis());
                for (int part = 0; part < parts.size(); part++) relay.add(new RelayPart(item.getString("id") + ":" + part, parts.get(part), item.getLong("receivedAtMs")));
            }
        } else {
            // Reauthorize even queued fragments so revoked grants cannot keep transmitting.
            pollRelay(new JSONObject().put("limit", 1));
        }
        final RelayPart part = relay.poll();
        if (part == null) return;
        if (System.currentTimeMillis() - part.receivedAtMs > 120_000) { error = "Stale relay traffic skipped"; return; }
        requireCurrent(current);
        if (!preferences.getBoolean("relayEnabled", false) || version != preferences.getLong("relayVersion", 0)) { cursor = null; relay.clear(); return; }
        final MeshBleSession radio = session;
        if (radio == null) return;
        if (markAttempt("mesh:" + part.id)) radio.command(3, part.payload);
        status = "Public relay accepted by radio; recipient delivery unconfirmed";
    }

    private JSONObject pollRelay(JSONObject payload) throws Exception {
        try { return requests.request("relay.poll", payload); }
        catch (Exception failure) { relay.clear(); cursor = null; throw failure; }
    }

    private boolean markAttempt(String id) throws Exception {
        final JSONArray old = new JSONArray(preferences.getString("attempted", "[]"));
        for (int i = 0; i < old.length(); i++) if (id.equals(old.getString(i))) return false;
        final JSONArray next = new JSONArray().put(id);
        for (int i = 0; i < Math.min(127, old.length()); i++) next.put(old.get(i));
        if (!preferences.edit().putString("attempted", next.toString()).commit()) throw new IOException("Could not save relay attempt; radio send cancelled");
        return true;
    }

    private void processingError(String operation, Exception failure) {
        error = "Received bytes archived; " + operation + " failed: " + failure.getMessage();
        Log.w("REM.goTenna", error, failure);
    }

    private void report(Exception failure) {
        error = failure.getMessage() == null ? failure.toString() : failure.getMessage();
        Log.w("REM.goTenna", "Radio/plugin action failed", failure);
        if (session != null && !session.isReady()) {
            disconnect();
            status = "Disconnected after radio failure";
        }
    }
}
