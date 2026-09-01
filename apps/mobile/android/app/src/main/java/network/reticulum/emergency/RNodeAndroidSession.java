package network.reticulum.emergency;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.IOException;
import java.util.Arrays;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

abstract class RNodeAndroidSession implements AutoCloseable {
    static final int MAX_CHUNK_BYTES = 4 * 1024;
    private static final int INBOUND_CAPACITY = 64;

    final long generation;
    final LinkedBlockingQueue<ReadEvent> inbound = new LinkedBlockingQueue<>(INBOUND_CAPACITY);
    final AtomicBoolean closed = new AtomicBoolean(false);
    final AtomicLong inboundChunks = new AtomicLong();
    final AtomicLong inboundBytes = new AtomicLong();
    final AtomicLong outboundChunks = new AtomicLong();
    final AtomicLong outboundBytes = new AtomicLong();
    final AtomicReference<String> lastError = new AtomicReference<>();

    RNodeAndroidSession(long generation) {
        this.generation = generation;
    }

    abstract void open(long timeoutMs) throws Exception;

    abstract String mode();

    abstract Integer negotiatedMtu();

    abstract void write(byte[] payload, long timeoutMs) throws Exception;

    @Override
    public abstract void close();

    byte[] read(long timeoutMs) throws Exception {
        final ReadEvent event = inbound.poll(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS);
        if (event == null) {
            return null;
        }
        if (event.error != null) {
            throw new IOException(event.error);
        }
        return event.payload;
    }

    String openResultJson() throws JSONException {
        final JSONObject result = new JSONObject();
        result.put("generation", generation);
        result.put("kind", mode());
        final Integer mtu = negotiatedMtu();
        result.put("negotiatedMtu", mtu == null ? JSONObject.NULL : mtu);
        return result.toString();
    }

    String statusJson() {
        try {
            final JSONObject result = new JSONObject();
            result.put("generation", generation);
            result.put("kind", mode());
            result.put("negotiatedMtu", negotiatedMtu());
            result.put("closed", closed.get());
            result.put("inboundChunks", inboundChunks.get());
            result.put("inboundBytes", inboundBytes.get());
            result.put("outboundChunks", outboundChunks.get());
            result.put("outboundBytes", outboundBytes.get());
            final String error = lastError.get();
            result.put("lastError", error == null ? JSONObject.NULL : error);
            return result.toString();
        } catch (JSONException error) {
            return "{\"generation\":" + generation + ",\"statusError\":\"json\"}";
        }
    }

    void recordWrite(byte[] payload) {
        outboundChunks.incrementAndGet();
        outboundBytes.addAndGet(payload.length);
    }

    void offerBytes(byte[] payload) {
        if (payload == null || payload.length == 0 || payload.length > MAX_CHUNK_BYTES) {
            if (payload != null && payload.length > MAX_CHUNK_BYTES) {
                fail("RNode inbound chunk exceeds 4096 bytes");
            }
            return;
        }
        if (!inbound.offer(ReadEvent.data(Arrays.copyOf(payload, payload.length)))) {
            fail("RNode inbound queue is full");
            return;
        }
        inboundChunks.incrementAndGet();
        inboundBytes.addAndGet(payload.length);
    }

    void fail(String message) {
        lastError.compareAndSet(null, message);
        close();
        inbound.clear();
        inbound.offer(ReadEvent.error(message));
    }

    static final class ReadEvent {
        final byte[] payload;
        final String error;

        private ReadEvent(byte[] payload, String error) {
            this.payload = payload;
            this.error = error;
        }

        static ReadEvent data(byte[] payload) {
            return new ReadEvent(payload, null);
        }

        static ReadEvent error(String error) {
            return new ReadEvent(null, error);
        }
    }
}
