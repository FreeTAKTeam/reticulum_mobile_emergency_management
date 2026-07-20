package network.reticulum.emergency;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

import androidx.core.content.ContextCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;
import com.getcapacitor.PermissionState;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;

import org.json.JSONException;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

public abstract class ReticulumNodePluginBase extends Plugin {
    protected static final String TAG = "ReticulumNode";
    static final String RNODE_BLUETOOTH_ALIAS = "rnodeBluetooth";
    private static final long SERVICE_BIND_TIMEOUT_MS = 10_000L;

    private volatile ReticulumNodeService boundService;
    private volatile boolean serviceBound = false;
    private volatile boolean serviceListenerRegistered = false;
    private volatile boolean bridgeForeground = true;
    private volatile RNodeBluetoothController rnodeBluetoothController;
    private volatile RNodeUsbPairingController rnodeUsbPairingController;
    private CompletableFuture<ReticulumNodeService> serviceFuture = new CompletableFuture<>();

    private final ExecutorService bridgeExecutor = Executors.newFixedThreadPool(4);
    private final ReticulumNodeService.ServiceEventListener serviceEventListener = (eventName, payload) -> {
        final ReticulumNodeService service = boundService;
        if (service != null && !service.isAppUiForeground()) {
            return;
        }
        final JSObject safePayload = payload == null ? new JSObject() : payload;
        if (!NativeEventBackpressure.shouldDispatchToUi(eventName, safePayload)) {
            return;
        }
        mirrorEventToLogcat(eventName, safePayload);
        notifyListeners(eventName, safePayload);
    };

    private final ServiceConnection serviceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder service) {
            if (!(service instanceof ReticulumNodeService.LocalBinder)) {
                Logger.error(TAG, "Unexpected binder for ReticulumNodeService", null);
                return;
            }

            final ReticulumNodeService.LocalBinder localBinder = (ReticulumNodeService.LocalBinder) service;
            boundService = localBinder.getService();
            serviceBound = true;
            boundService.setAppUiForeground(bridgeForeground);
            tryRegisterServiceListener();
            serviceFuture.complete(boundService);
            Logger.info(TAG, "Bound to ReticulumNodeService.");
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            Logger.info(TAG, "ReticulumNodeService disconnected.");
        }

        @Override
        public void onBindingDied(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            bindToService();
        }

        @Override
        public void onNullBinding(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            Logger.error(TAG, "ReticulumNodeService returned null binding.", null);
        }
    };

    @Override
    public void load() {
        super.load();
        bridgeForeground = true;
        Logger.info(TAG, "ReticulumNode plugin loaded.");
    }

    @Override
    protected void handleOnResume() {
        super.handleOnResume();
        bridgeForeground = true;
        if (boundService != null) {
            boundService.setAppUiForeground(true);
        }
        tryRegisterServiceListener();
    }

    @Override
    protected void handleOnPause() {
        bridgeForeground = false;
        if (boundService != null) {
            boundService.setAppUiForeground(false);
        }
        unregisterServiceListener();
        super.handleOnPause();
    }

    @Override
    protected void handleOnStop() {
        bridgeForeground = false;
        if (boundService != null) {
            boundService.setAppUiForeground(false);
        }
        unregisterServiceListener();
        super.handleOnStop();
    }

    @Override
    protected void handleOnDestroy() {
        bridgeForeground = false;
        if (boundService != null) {
            boundService.setAppUiForeground(false);
        }
        unregisterServiceListener();
        final RNodeUsbPairingController pairingController = rnodeUsbPairingController;
        if (pairingController != null) {
            pairingController.close();
        }
        unbindFromService();
        bridgeExecutor.shutdownNow();
        super.handleOnDestroy();
    }

    protected final void executeBridgeTask(Runnable task) {
        bridgeExecutor.execute(task);
    }

    protected final void startServiceForRuntime() {
        final Context appContext = getContext().getApplicationContext();
        final Intent serviceIntent = new Intent(appContext, ReticulumNodeService.class);
        serviceIntent.setAction(ReticulumNodeService.ACTION_START_RUNTIME);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ContextCompat.startForegroundService(appContext, serviceIntent);
        } else {
            appContext.startService(serviceIntent);
        }
        bindToService();
    }

    protected final RNodeBluetoothController rnodeBluetoothController() {
        RNodeBluetoothController current = rnodeBluetoothController;
        if (current == null) {
            synchronized (this) {
                current = rnodeBluetoothController;
                if (current == null) {
                    current = new RNodeBluetoothController(getContext(), this::notifyListeners);
                    rnodeBluetoothController = current;
                }
            }
        }
        return current;
    }

    protected final RNodeUsbPairingController rnodeUsbPairingController() {
        RNodeUsbPairingController current = rnodeUsbPairingController;
        if (current == null) {
            synchronized (this) {
                current = rnodeUsbPairingController;
                if (current == null) {
                    current = new RNodeUsbPairingController(
                        getContext(),
                        bridgeExecutor,
                        rnodeBluetoothController(),
                        this::notifyListeners
                    );
                    rnodeUsbPairingController = current;
                }
            }
        }
        return current;
    }

    protected final void runIntServiceCall(
        PluginCall call,
        String fallbackMessage,
        ServiceIntOperation operation
    ) {
        runIntServiceCall(call, fallbackMessage, operation, null);
    }

    protected final void runIntServiceCall(
        PluginCall call,
        String fallbackMessage,
        ServiceIntOperation operation,
        Runnable onSuccess
    ) {
        bridgeExecutor.execute(() -> {
            try {
                final ReticulumNodeService service = awaitService();
                final int result = operation.run(service);
                if (result != 0) {
                    rejectFromNative(call, fallbackMessage);
                    return;
                }
                if (onSuccess != null) {
                    onSuccess.run();
                }
                call.resolve();
            } catch (Exception ex) {
                call.reject(fallbackMessage, ex);
            }
        });
    }

    protected final void runStringServiceCall(
        PluginCall call,
        String fallbackMessage,
        String parseFallbackMessage,
        ServiceStringOperation operation
    ) {
        bridgeExecutor.execute(() -> {
            try {
                final ReticulumNodeService service = awaitService();
                final String raw = operation.run(service);
                if (raw == null || raw.isEmpty()) {
                    rejectFromNative(call, fallbackMessage);
                    return;
                }
                resolveJson(call, raw, parseFallbackMessage);
            } catch (Exception ex) {
                call.reject(fallbackMessage, ex);
            }
        });
    }

    protected final void resolveRnodeBluetoothPermission(PluginCall call) {
        final RNodeBluetoothController controller = rnodeBluetoothController();
        final PermissionState state = controller.hasPermission()
            ? PermissionState.GRANTED
            : getPermissionState(RNODE_BLUETOOTH_ALIAS);
        controller.resolvePermission(call, state);
    }

    protected final void rejectFromNative(PluginCall call, String fallbackMessage) {
        final String raw = ReticulumBridge.takeLastErrorJson();
        if (raw == null || raw.isEmpty()) {
            Logger.error(TAG, fallbackMessage, new Exception(fallbackMessage));
            call.reject(fallbackMessage);
            return;
        }

        try {
            final JSObject payload = new JSObject(raw);
            final String code = payload.getString("code", "NativeError");
            final String message = payload.getString("message", fallbackMessage);
            Log.e(TAG, "rejectFromNative code=" + code + " message=" + message);
            Logger.error(TAG, "Native error [" + code + "]: " + message, new Exception(message));
            final JSObject details = new JSObject();
            details.put("code", code);
            details.put("message", message);
            details.put("retryable", payload.getBoolean("retryable", false));
            if (payload.has("operation")) {
                details.put("operation", payload.getString("operation"));
            }
            if (payload.has("cause")) {
                details.put("cause", payload.getString("cause"));
            }
            call.reject(message, code, details);
        } catch (JSONException ex) {
            call.reject(fallbackMessage, ex);
        }
    }

    protected interface ServiceIntOperation {
        int run(ReticulumNodeService service) throws Exception;
    }

    protected interface ServiceStringOperation {
        String run(ReticulumNodeService service) throws Exception;
    }

    private void bindToService() {
        if (serviceBound) {
            return;
        }
        final Context appContext = getContext().getApplicationContext();
        final Intent serviceIntent = new Intent(appContext, ReticulumNodeService.class);
        final boolean bound = appContext.bindService(serviceIntent, serviceConnection, Context.BIND_AUTO_CREATE);
        if (!bound) {
            Logger.error(TAG, "Failed to bind ReticulumNodeService.", null);
        }
    }

    private void unbindFromService() {
        if (!serviceBound) {
            return;
        }
        final Context appContext = getContext().getApplicationContext();
        appContext.unbindService(serviceConnection);
        serviceBound = false;
        boundService = null;
        resetServiceFuture();
    }

    protected final ReticulumNodeService awaitService() throws Exception {
        bindToService();
        return serviceFuture.get(SERVICE_BIND_TIMEOUT_MS, TimeUnit.MILLISECONDS);
    }

    private void tryRegisterServiceListener() {
        if (boundService == null || serviceListenerRegistered || !bridgeForeground) {
            return;
        }
        boundService.addListener(serviceEventListener);
        serviceListenerRegistered = true;
    }

    private void unregisterServiceListener() {
        if (boundService == null || !serviceListenerRegistered) {
            return;
        }
        boundService.removeListener(serviceEventListener);
        serviceListenerRegistered = false;
    }

    private void resetServiceFuture() {
        serviceFuture = new CompletableFuture<>();
    }

    private void mirrorEventToLogcat(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            final String level = payload.getString("level", "Info");
            final String message = payload.getString("message", payload.toString());
            writeLogcat(level, message);
            return;
        }

        if (
            "lxmfDelivery".equals(eventName)
                || "packetReceived".equals(eventName)
                || "packetSent".equals(eventName)
                || "announceReceived".equals(eventName)
        ) {
            Log.i(TAG, "[" + eventName + "] " + abbreviate(payload.toString()));
        }
    }

    protected final void writeLogcat(String level, String message) {
        final int priority;
        switch (level) {
            case "Trace":
            case "Debug":
                priority = Log.DEBUG;
                break;
            case "Warn":
                priority = Log.WARN;
                break;
            case "Error":
                priority = Log.ERROR;
                break;
            case "Info":
            default:
                priority = Log.INFO;
                break;
        }

        Log.println(priority, TAG, abbreviate(message));
    }

    private String abbreviate(String value) {
        if (value == null) {
            return "";
        }
        final int maxLength = 4000;
        if (value.length() <= maxLength) {
            return value;
        }
        return value.substring(0, maxLength) + "...";
    }

    private void resolveJson(PluginCall call, String raw, String fallbackMessage) {
        try {
            call.resolve(new JSObject(raw));
        } catch (JSONException ex) {
            call.reject(fallbackMessage, ex);
        }
    }
}
