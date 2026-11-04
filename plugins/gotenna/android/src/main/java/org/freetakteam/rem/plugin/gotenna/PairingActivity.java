package org.freetakteam.rem.plugin.gotenna;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.Activity;
import android.app.AlertDialog;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothManager;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanFilter;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelUuid;
import android.util.Log;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class PairingActivity extends Activity {
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Map<String, String> devices = new LinkedHashMap<>();
    private BluetoothLeScanner scanner;
    private AlertDialog dialog;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        final String[] permissions = Build.VERSION.SDK_INT >= 31
            ? new String[] {Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT}
            : new String[] {Manifest.permission.ACCESS_FINE_LOCATION};
        for (String permission : permissions) {
            if (checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(permissions, 1);
                return;
            }
        }
        scan();
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] grants) {
        super.onRequestPermissionsResult(request, permissions, grants);
        if (request != 1) return;
        if (grants.length == 0) { finish(); return; }
        for (int grant : grants) {
            if (grant != PackageManager.PERMISSION_GRANTED) { showError("Bluetooth permissions are required."); return; }
        }
        scan();
    }

    @SuppressLint("MissingPermission")
    private void scan() {
        final BluetoothManager manager = getSystemService(BluetoothManager.class);
        final BluetoothAdapter adapter = manager == null ? null : manager.getAdapter();
        if (adapter == null || !adapter.isEnabled()) { showError("Turn Bluetooth on first."); return; }
        scanner = adapter.getBluetoothLeScanner();
        if (scanner == null) { showError("Bluetooth scanning is unavailable."); return; }
        dialog = new AlertDialog.Builder(this).setTitle("Find goTenna Mesh")
            .setMessage("Put your Mesh in pairing mode. Searching for 15 seconds…")
            .setNegativeButton(android.R.string.cancel, (d, w) -> finish()).setOnCancelListener(d -> finish()).show();
        try {
            scanner.startScan(Collections.singletonList(new ScanFilter.Builder().setServiceUuid(new ParcelUuid(MeshBleSession.SERVICE)).build()),
                new ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(), callback);
            handler.postDelayed(this::choose, 15_000);
        } catch (Exception error) { showError(error.getMessage()); }
    }

    private final ScanCallback callback = new ScanCallback() {
        @SuppressLint("MissingPermission")
        @Override public void onScanResult(int type, ScanResult result) {
            devices.put(result.getDevice().getAddress(), result.getDevice().getName() == null ? "goTenna Mesh" : result.getDevice().getName());
        }
        @Override public void onScanFailed(int code) { showError("Bluetooth scan failed (" + code + ")"); }
    };

    private void choose() {
        stopScan();
        if (isFinishing()) return;
        if (devices.isEmpty()) { showError("No Mesh found. Enable location on Android 8–11 and close other goTenna apps."); return; }
        if (dialog != null) dialog.dismiss();
        final List<String> addresses = new ArrayList<>(devices.keySet());
        final String[] labels = new String[addresses.size()];
        for (int i = 0; i < labels.length; i++) labels[i] = devices.get(addresses.get(i)) + "\n" + addresses.get(i);
        dialog = new AlertDialog.Builder(this).setTitle("Select your Mesh").setItems(labels, (d, which) -> {
            final String address = addresses.get(which);
            if (!getSharedPreferences("gotenna", MODE_PRIVATE).edit().putString("address", address)
                .putString("deviceName", devices.get(address)).commit()) { showError("Could not save selected radio"); return; }
            finish();
        }).setNegativeButton(android.R.string.cancel, (d, w) -> finish()).setOnCancelListener(d -> finish()).show();
    }

    private void showError(String message) {
        stopScan();
        if (isFinishing()) return;
        if (dialog != null) dialog.dismiss();
        dialog = new AlertDialog.Builder(this).setTitle("Mesh pairing").setMessage(message)
            .setPositiveButton(android.R.string.ok, (d, w) -> finish()).setOnCancelListener(d -> finish()).show();
    }

    @SuppressLint("MissingPermission")
    private void stopScan() {
        handler.removeCallbacksAndMessages(null);
        if (scanner != null) {
            try { scanner.stopScan(callback); } catch (Exception error) { Log.w("REM.goTenna", "Scan cleanup failed", error); }
            scanner = null;
        }
    }

    @Override protected void onDestroy() {
        stopScan();
        if (dialog != null) dialog.dismiss();
        super.onDestroy();
    }
}
