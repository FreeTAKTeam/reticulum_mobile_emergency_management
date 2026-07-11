package network.reticulum.emergency.plugins;

import android.app.Activity;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.pm.ResolveInfo;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.os.IBinder;
import android.webkit.CookieManager;
import android.webkit.MimeTypeMap;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import androidx.annotation.Nullable;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import network.reticulum.emergency.plugin.api.CallerCertificateVerifier;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginService;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONObject;

public final class PluginConfigurationActivity extends Activity implements ServiceConnection {
    private static final String ORIGIN = "https://appassets.androidplatform.net";
    private static final String PREFIX = "/rem-plugin/";
    private WebView webView;
    private WebMessagePort nativePort;
    private IRemPluginService service;
    private String packageName;
    private String serviceClassName;
    private String entrypoint;
    private String assetDirectory;
    private Context pluginContext;
    private boolean bound;

    public static Intent intentFor(Context context, JSONObject plugin) {
        return new Intent(context, PluginConfigurationActivity.class)
            .putExtra("pluginId", plugin.optString("pluginId"))
            .putExtra("pluginPackage", plugin.optString("packageName"))
            .putExtra("pluginService", plugin.optString("serviceClassName"))
            .putExtra("pluginEntrypoint", plugin.optString("configurationEntrypoint"))
            .putExtra("pluginFingerprint", plugin.optString("publisherFingerprint"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
    }

    @Override
    protected void onCreate(@Nullable Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        packageName = getIntent().getStringExtra("pluginPackage");
        serviceClassName = getIntent().getStringExtra("pluginService");
        entrypoint = sanitizeAssetPath(getIntent().getStringExtra("pluginEntrypoint"));
        assetDirectory = entrypoint == null || !entrypoint.contains("/")
            ? null
            : entrypoint.substring(0, entrypoint.lastIndexOf('/'));
        final String expectedFingerprint = CallerCertificateVerifier.normalizeFingerprint(
            getIntent().getStringExtra("pluginFingerprint")
        );
        if (packageName == null
            || serviceClassName == null
            || entrypoint == null
            || assetDirectory == null
            || expectedFingerprint.isEmpty()) {
            finish();
            return;
        }
        final Set<String> currentFingerprints = CallerCertificateVerifier.currentPackageFingerprints(
            this,
            packageName
        );
        if (currentFingerprints.size() != 1
            || !currentFingerprints.contains(expectedFingerprint)) {
            finish();
            return;
        }
        try {
            pluginContext = createPackageContext(packageName, Context.CONTEXT_IGNORE_SECURITY);
        } catch (Exception error) {
            finish();
            return;
        }
        configureWebView();
        setContentView(webView);
        bound = bindService(
            new Intent(PluginProtocol.SERVICE_ACTION).setComponent(
                new ComponentName(packageName, serviceClassName)
            ),
            this,
            Context.BIND_AUTO_CREATE
        );
        webView.loadUrl(ORIGIN + PREFIX + entrypoint);
    }

    private void configureWebView() {
        webView = new WebView(this);
        webView.setBackgroundColor(Color.TRANSPARENT);
        final WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setDomStorageEnabled(false);
        settings.setDatabaseEnabled(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setGeolocationEnabled(false);
        settings.setCacheMode(WebSettings.LOAD_NO_CACHE);
        settings.setJavaScriptCanOpenWindowsAutomatically(false);
        settings.setSupportMultipleWindows(false);
        final CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(false);
        cookies.setAcceptThirdPartyCookies(webView, false);
        cookies.removeAllCookies(null);
        webView.clearCache(true);
        webView.clearHistory();
        webView.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                return !isLocal(request.getUrl());
            }

            @Override
            public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                final Uri uri = request.getUrl();
                if (!isLocal(uri)) {
                    return emptyResponse();
                }
                final String path = sanitizeAssetPath(uri.getPath().substring(PREFIX.length()));
                if (path == null || !path.startsWith(assetDirectory + "/")) {
                    return emptyResponse();
                }
                try {
                    final InputStream input = pluginContext.getAssets().open(path);
                    final Map<String, String> headers = new HashMap<>();
                    headers.put(
                        "Content-Security-Policy",
                        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'"
                    );
                    headers.put("X-Content-Type-Options", "nosniff");
                    return new WebResourceResponse(
                        mimeType(path),
                        "UTF-8",
                        200,
                        "OK",
                        headers,
                        input
                    );
                } catch (IOException error) {
                    return emptyResponse();
                }
            }

            @Override
            public void onPageFinished(WebView view, String url) {
                establishMessageChannel();
            }
        });
    }

    private void establishMessageChannel() {
        if (nativePort != null || service == null) {
            return;
        }
        final WebMessagePort[] ports = webView.createWebMessageChannel();
        nativePort = ports[0];
        nativePort.setWebMessageCallback(new WebMessagePort.WebMessageCallback() {
            @Override
            public void onMessage(WebMessagePort port, WebMessage message) {
                handleWebMessage(message.getData());
            }
        });
        webView.postWebMessage(
            new WebMessage("rem-plugin-config-v1", new WebMessagePort[] {ports[1]}),
            Uri.parse(ORIGIN)
        );
    }

    private void handleWebMessage(String raw) {
        if (raw == null || raw.getBytes(StandardCharsets.UTF_8).length > PluginProtocol.MAX_JSON_BYTES) {
            return;
        }
        try {
            final JSONObject message = new JSONObject(raw);
            final String type = message.optString("type", "");
            if (!type.equals("ready")
                && !type.equals("getState")
                && !type.equals("update")
                && !type.equals("action")) {
                return;
            }
            service.handleConfigurationRequest(
                raw,
                new IRemPluginConfigurationCallback.Stub() {
                    @Override
                    public void onResponse(String responseJson) {
                        runOnUiThread(() -> {
                            if (nativePort != null && responseJson != null) {
                                try {
                                    PluginProtocol.requireJsonSize(
                                        responseJson,
                                        "Plugin configuration response"
                                    );
                                    handleConfigurationResponse(responseJson);
                                    nativePort.postMessage(new WebMessage(responseJson));
                                } catch (Exception ignored) {
                                }
                            }
                        });
                    }
                }
            );
        } catch (Exception ignored) {
        }
    }

    @Override
    public void onServiceConnected(ComponentName name, IBinder binder) {
        service = IRemPluginService.Stub.asInterface(binder);
        establishMessageChannel();
    }

    private void handleConfigurationResponse(String responseJson) {
        try {
            final JSONObject response = new JSONObject(responseJson);
            if (!"actionResult".equals(response.optString("type"))) {
                return;
            }
            final JSONObject activity = response.optJSONObject("activity");
            if (activity == null) {
                return;
            }
            final String className = activity.optString("className", "").trim();
            if (className.isEmpty()) {
                return;
            }
            final Intent intent = new Intent().setComponent(
                new ComponentName(packageName, className)
            );
            final ResolveInfo resolved = getPackageManager().resolveActivity(intent, 0);
            if (resolved == null
                || resolved.activityInfo == null
                || !packageName.equals(resolved.activityInfo.packageName)) {
                return;
            }
            startActivity(intent);
        } catch (Exception ignored) {
        }
    }

    @Override
    public void onServiceDisconnected(ComponentName name) {
        service = null;
        finish();
    }

    @Override
    protected void onDestroy() {
        if (bound) {
            try {
                unbindService(this);
            } catch (IllegalArgumentException ignored) {
            }
            bound = false;
        }
        if (nativePort != null) {
            nativePort.close();
        }
        if (webView != null) {
            webView.destroy();
        }
        super.onDestroy();
    }

    private boolean isLocal(Uri uri) {
        return "https".equals(uri.getScheme())
            && "appassets.androidplatform.net".equals(uri.getHost())
            && uri.getPath() != null
            && uri.getPath().startsWith(PREFIX);
    }

    private static String sanitizeAssetPath(String value) {
        if (value == null) {
            return null;
        }
        final String normalized = value.replace('\\', '/');
        if (normalized.isEmpty() || normalized.startsWith("/") || normalized.contains("../")) {
            return null;
        }
        return normalized;
    }

    private static String mimeType(String path) {
        final String extension = MimeTypeMap.getFileExtensionFromUrl(path);
        final String value = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension);
        return value == null ? "application/octet-stream" : value;
    }

    private static WebResourceResponse emptyResponse() {
        return new WebResourceResponse(
            "text/plain",
            "UTF-8",
            403,
            "Blocked",
            Collections.emptyMap(),
            new ByteArrayInputStream(new byte[0])
        );
    }
}
