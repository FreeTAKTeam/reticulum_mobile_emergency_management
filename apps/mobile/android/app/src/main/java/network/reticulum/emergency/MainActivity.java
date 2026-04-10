package network.reticulum.emergency;

import android.os.Bundle;
import android.util.Log;
import com.getcapacitor.BridgeActivity;

public class MainActivity extends BridgeActivity {
    private static final String TAG = "ReticulumNode";
    private static final String CAPACITOR_TRIGGER_EVENT_SHIM =
        "window.Capacitor = window.Capacitor || {};" +
        "if (typeof window.Capacitor.triggerEvent !== 'function') {" +
        "window.Capacitor.triggerEvent = function(){ return false; };" +
        "}";

    @Override
    public void onCreate(Bundle savedInstanceState) {
        // Register custom plugin before bridge startup.
        registerPlugin(ReticulumNodePlugin.class);
        super.onCreate(savedInstanceState);
        installCapacitorTriggerEventShim();
        Log.i(TAG, "MainActivity initialized and ReticulumNode plugin registered.");
    }

    @Override
    public void onResume() {
        super.onResume();
        installCapacitorTriggerEventShim();
    }

    private void installCapacitorTriggerEventShim() {
        if (bridge == null || bridge.getWebView() == null) {
            return;
        }
        bridge.getWebView().post(() -> bridge.getWebView().evaluateJavascript(CAPACITOR_TRIGGER_EVENT_SHIM, null));
    }
}
