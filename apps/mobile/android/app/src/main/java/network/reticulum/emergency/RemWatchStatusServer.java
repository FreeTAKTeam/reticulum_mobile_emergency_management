package network.reticulum.emergency;

import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

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

final class RemWatchStatusServer {
    interface StatusProvider {
        String getStatusJson();
    }

    private static final String TAG = "RemWatchStatusServer";
    private static final String LOOPBACK_HOST = "127.0.0.1";

    private final Object lock = new Object();
    private ServerSocket serverSocket;
    private ExecutorService connectionExecutor;
    private Thread acceptThread;
    private RemWatchStatusServerSettings settings = RemWatchStatusServerSettings.defaults();
    private String bindError = "";

    void apply(RemWatchStatusServerSettings nextSettings, StatusProvider provider) {
        synchronized (lock) {
            settings = nextSettings == null ? RemWatchStatusServerSettings.defaults() : nextSettings;
            bindError = "";
            stopLocked();
            if (!settings.enabled) {
                return;
            }
            try {
                serverSocket = new ServerSocket();
                serverSocket.bind(new InetSocketAddress(InetAddress.getByName(LOOPBACK_HOST), settings.port));
                connectionExecutor = Executors.newCachedThreadPool();
                acceptThread = new Thread(() -> acceptLoop(provider), "rem-watch-status-server");
                acceptThread.setDaemon(true);
                acceptThread.start();
                Log.i(TAG, "REM watch status server listening on " + settings.url());
            } catch (IOException ex) {
                bindError = ex.getMessage() == null ? ex.toString() : ex.getMessage();
                stopLocked();
                Log.w(TAG, "REM watch status server failed to bind: " + bindError);
            }
        }
    }

    void stop() {
        synchronized (lock) {
            stopLocked();
        }
    }

    boolean isRunning() {
        synchronized (lock) {
            return serverSocket != null && !serverSocket.isClosed() && bindError.isEmpty();
        }
    }

    String bindError() {
        synchronized (lock) {
            return bindError;
        }
    }

    JSONObject stateJson() throws JSONException {
        synchronized (lock) {
            return settings.toJson(isRunning(), bindError);
        }
    }

    private void stopLocked() {
        if (serverSocket != null) {
            try {
                serverSocket.close();
            } catch (IOException ex) {
                Log.d(TAG, "Ignoring watch status server close failure.", ex);
            }
            serverSocket = null;
        }
        if (connectionExecutor != null) {
            connectionExecutor.shutdownNow();
            connectionExecutor = null;
        }
        acceptThread = null;
    }

    private void acceptLoop(StatusProvider provider) {
        while (true) {
            final ServerSocket activeSocket;
            final ExecutorService executor;
            synchronized (lock) {
                activeSocket = serverSocket;
                executor = connectionExecutor;
            }
            if (activeSocket == null || activeSocket.isClosed() || executor == null) {
                return;
            }
            try {
                final Socket socket = activeSocket.accept();
                executor.execute(() -> handle(socket, provider));
            } catch (IOException ex) {
                if (activeSocket.isClosed()) {
                    return;
                }
                Log.w(TAG, "REM watch status accept failed.", ex);
            }
        }
    }

    private void handle(Socket socket, StatusProvider provider) {
        try (Socket client = socket) {
            final BufferedReader reader = new BufferedReader(
                new InputStreamReader(client.getInputStream(), StandardCharsets.US_ASCII)
            );
            final String requestLine = reader.readLine();
            final String path = requestPath(requestLine);
            String headerLine;
            while ((headerLine = reader.readLine()) != null) {
                if ("".equals(headerLine)) {
                    break;
                }
            }

            if ("/info.json".equals(path) || "/northbound/watch/status".equals(path)) {
                writeResponse(client.getOutputStream(), 200, "application/json", provider.getStatusJson());
                return;
            }
            if ("/health".equals(path)) {
                writeResponse(client.getOutputStream(), 200, "application/json", "{\"ok\":true}");
                return;
            }
            writeResponse(client.getOutputStream(), 404, "text/plain", "not found");
        } catch (IOException ex) {
            Log.d(TAG, "REM watch status request failed.", ex);
        }
    }

    private static String requestPath(String requestLine) {
        if (requestLine == null) {
            return "";
        }
        final String[] parts = requestLine.split(" ");
        if (parts.length < 2 || !"GET".equals(parts[0].toUpperCase(Locale.US))) {
            return "";
        }
        return parts[1];
    }

    private static void writeResponse(OutputStream output, int status, String contentType, String body) throws IOException {
        final byte[] bytes = String.valueOf(body).getBytes(StandardCharsets.UTF_8);
        final String reason = status == 200 ? "OK" : "Not Found";
        final String headers =
            "HTTP/1.1 " + status + " " + reason + "\r\n"
                + "Content-Type: " + contentType + "; charset=utf-8\r\n"
                + "Content-Length: " + bytes.length + "\r\n"
                + "Connection: close\r\n"
                + "\r\n";
        output.write(headers.getBytes(StandardCharsets.US_ASCII));
        output.write(bytes);
        output.flush();
    }
}
