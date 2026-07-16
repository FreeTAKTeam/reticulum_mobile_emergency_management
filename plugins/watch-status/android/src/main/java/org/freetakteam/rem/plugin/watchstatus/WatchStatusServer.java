package org.freetakteam.rem.plugin.watchstatus;

import android.util.Log;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class WatchStatusServer {
    interface StatusProvider { String getStatusJson(); }
    private static final String TAG = "REM.WatchStatus";
    private final Object lock = new Object();
    private ServerSocket socket;
    private ExecutorService connections;
    private String bindError = "";

    void apply(boolean enabled, int port, StatusProvider provider) {
        synchronized (lock) {
            stopLocked();
            bindError = "";
            if (!enabled) return;
            try {
                socket = new ServerSocket();
                socket.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"), port));
                connections = Executors.newCachedThreadPool();
                final Thread acceptThread = new Thread(() -> acceptLoop(provider), "rem-watch-status-server");
                acceptThread.setDaemon(true);
                acceptThread.start();
            } catch (IOException error) {
                bindError = error.getMessage() == null ? error.toString() : error.getMessage();
                stopLocked();
                Log.w(TAG, "Failed to bind watch status server", error);
            }
        }
    }

    void stop() { synchronized (lock) { stopLocked(); } }
    boolean isRunning() { synchronized (lock) { return socket != null && !socket.isClosed() && bindError.isEmpty(); } }
    String bindError() { synchronized (lock) { return bindError; } }

    private void stopLocked() {
        if (socket != null) {
            try { socket.close(); } catch (IOException ignored) {}
            socket = null;
        }
        if (connections != null) connections.shutdownNow();
        connections = null;
    }

    private void acceptLoop(StatusProvider provider) {
        while (true) {
            final ServerSocket active;
            final ExecutorService executor;
            synchronized (lock) { active = socket; executor = connections; }
            if (active == null || active.isClosed() || executor == null) return;
            try {
                final Socket client = active.accept();
                executor.execute(() -> handle(client, provider));
            } catch (IOException error) {
                if (active.isClosed()) return;
                Log.w(TAG, "Watch status accept failed", error);
            }
        }
    }

    private void handle(Socket socket, StatusProvider provider) {
        try (Socket client = socket) {
            final BufferedReader reader = new BufferedReader(new InputStreamReader(client.getInputStream(), StandardCharsets.US_ASCII));
            final String path = requestPath(reader.readLine());
            String line;
            while ((line = reader.readLine()) != null && !line.isEmpty()) {}
            if ("/info.json".equals(path) || "/northbound/watch/status".equals(path)) {
                write(client.getOutputStream(), 200, "application/json", provider.getStatusJson());
            } else if ("/health".equals(path)) {
                write(client.getOutputStream(), 200, "application/json", "{\"ok\":true}");
            } else {
                write(client.getOutputStream(), 404, "text/plain", "not found");
            }
        } catch (IOException error) {
            Log.d(TAG, "Watch status request failed", error);
        }
    }

    static String requestPath(String requestLine) {
        if (requestLine == null) return "";
        final String[] parts = requestLine.split(" ");
        return parts.length >= 2 && "GET".equals(parts[0].toUpperCase(Locale.US)) ? parts[1] : "";
    }

    private static void write(OutputStream output, int status, String contentType, String body) throws IOException {
        final byte[] bytes = String.valueOf(body).getBytes(StandardCharsets.UTF_8);
        final String reason = status == 200 ? "OK" : "Not Found";
        final String headers = "HTTP/1.1 " + status + " " + reason + "\r\nContent-Type: " + contentType
            + "; charset=utf-8\r\nContent-Length: " + bytes.length + "\r\nConnection: close\r\n\r\n";
        output.write(headers.getBytes(StandardCharsets.US_ASCII));
        output.write(bytes);
        output.flush();
    }
}
