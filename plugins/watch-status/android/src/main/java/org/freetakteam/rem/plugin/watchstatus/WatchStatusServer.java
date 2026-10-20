package org.freetakteam.rem.plugin.watchstatus;

import android.util.Log;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

final class WatchStatusServer {
    interface StatusProvider { String getStatusJson(); }
    private static final String TAG = "REM.WatchStatus";
    private final Object lock = new Object();
    private final Set<Socket> clients = ConcurrentHashMap.newKeySet();
    private ServerSocket socket;
    private ThreadPoolExecutor connections;
    private String bindError = "";

    void apply(boolean enabled, int port, StatusProvider provider) { apply(enabled, port, provider, null); }

    void apply(boolean enabled, int port, StatusProvider provider, RadarProvider radar) {
        synchronized (lock) {
            stopLocked();
            bindError = "";
            if (!enabled) return;
            try {
                socket = new ServerSocket();
                socket.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"), port), 8);
                connections = new ThreadPoolExecutor(4, 4, 0, TimeUnit.MILLISECONDS,
                    new ArrayBlockingQueue<>(4), runnable -> {
                        final Thread thread = new Thread(runnable, "rem-watch-http");
                        thread.setDaemon(true);
                        return thread;
                    });
                final ServerSocket active = socket;
                final ThreadPoolExecutor executor = connections;
                final Thread acceptThread = new Thread(() -> acceptLoop(active, executor, provider, radar), "rem-watch-status-server");
                acceptThread.setDaemon(true);
                acceptThread.start();
            } catch (IOException error) {
                bindError = "Loopback bind failed";
                stopLocked();
                Log.w(TAG, "Failed to bind watch status server");
            }
        }
    }

    void stop() { synchronized (lock) { stopLocked(); } }
    boolean isRunning() { synchronized (lock) { return socket != null && !socket.isClosed() && bindError.isEmpty(); } }
    String bindError() { synchronized (lock) { return bindError; } }

    private void stopLocked() {
        close(socket);
        socket = null;
        for (Socket client : clients) close(client);
        clients.clear();
        if (connections != null) connections.shutdownNow();
        connections = null;
    }

    private void acceptLoop(ServerSocket active, ThreadPoolExecutor executor, StatusProvider provider, RadarProvider radar) {
        while (!active.isClosed()) {
            try {
                final Socket client = active.accept();
                clients.add(client);
                try { executor.execute(() -> handle(client, provider, radar)); }
                catch (RejectedExecutionException full) { clients.remove(client); close(client); }
            } catch (IOException error) {
                if (!active.isClosed()) Log.w(TAG, "Watch status accept failed");
            }
        }
    }

    private void handle(Socket socket, StatusProvider provider, RadarProvider radar) {
        try (Socket client = socket) {
            client.setSoTimeout(500);
            final HttpInput input = new HttpInput(client.getInputStream());
            final String path = requestPath(input.line());
            final Map<String, String> headers = new HashMap<>();
            String line;
            while (!(line = input.line()).isEmpty()) {
                final int colon = line.indexOf(':');
                if (colon <= 0 || headers.size() >= 32) throw new IOException("Invalid headers");
                final String key = line.substring(0, colon).toLowerCase(Locale.ROOT);
                if (headers.put(key, line.substring(colon + 1).trim()) != null) throw new IOException("Duplicate header");
            }
            if ("/northbound/watch/radar".equals(path)) {
                final String requestId = headers.getOrDefault("x-rem-radar-request", "");
                try {
                    if (radar == null) throw new RadarFailure("RadarDisabled", 403);
                    write(client.getOutputStream(), 200, "application/json", radar.get(headers.get("authorization"), requestId));
                } catch (RadarFailure error) {
                    write(client.getOutputStream(), error.httpStatus, "application/json", error.json(
                        requestId.matches("[A-Za-z0-9_-]{1,64}") ? requestId : ""));
                }
            } else if ("/info.json".equals(path) || "/northbound/watch/status".equals(path)) {
                write(client.getOutputStream(), 200, "application/json", provider.getStatusJson());
            } else if ("/health".equals(path)) {
                write(client.getOutputStream(), 200, "application/json", "{\"ok\":true}");
            } else {
                write(client.getOutputStream(), 404, "text/plain", "not found");
            }
        } catch (IOException error) {
            // Header deadlines, overload and disconnects never read a location provider.
        } finally {
            clients.remove(socket);
        }
    }

    static String requestPath(String requestLine) {
        if (requestLine == null) return "";
        final String[] parts = requestLine.split(" ");
        return parts.length == 3 && "GET".equals(parts[0]) && "HTTP/1.1".equals(parts[2]) ? parts[1] : "";
    }

    private static void write(OutputStream output, int status, String contentType, String body) throws IOException {
        final byte[] bytes = String.valueOf(body).getBytes(StandardCharsets.UTF_8);
        if (bytes.length > 65_536) throw new IOException("Response too large");
        final String headers = "HTTP/1.1 " + status + " " + (status == 200 ? "OK" : "Error")
            + "\r\nContent-Type: " + contentType + "; charset=utf-8\r\nContent-Length: " + bytes.length
            + "\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
        output.write(headers.getBytes(StandardCharsets.US_ASCII));
        output.write(bytes);
        output.flush();
    }

    private static void close(java.io.Closeable resource) {
        if (resource == null) return;
        try { resource.close(); } catch (IOException ignored) { /* Already disconnected. */ }
    }

    private static final class HttpInput {
        private final InputStream stream;
        private final long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(2);
        private int count;
        HttpInput(InputStream stream) { this.stream = stream; }
        String line() throws IOException {
            final StringBuilder value = new StringBuilder();
            boolean cr = false;
            while (++count <= 8_192 && value.length() <= 2_048 && System.nanoTime() < deadline) {
                final int b = stream.read();
                if (b < 0) throw new IOException("Incomplete header");
                if (cr) {
                    if (b != '\n') throw new IOException("Invalid line ending");
                    return value.toString();
                }
                if (b == '\r') cr = true;
                else if (b < 32 || b > 126) throw new IOException("Invalid header character");
                else value.append((char) b);
            }
            throw new IOException("Header limit exceeded");
        }
    }
}
