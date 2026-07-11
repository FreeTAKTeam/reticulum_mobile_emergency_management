package org.freetakteam.rem.plugin.bleheartrate;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.Activity;
import android.app.AlertDialog;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanFilter;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.companion.AssociationRequest;
import android.companion.BluetoothLeDeviceFilter;
import android.companion.CompanionDeviceManager;
import android.content.Intent;
import android.content.IntentSender;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelUuid;
import androidx.annotation.Nullable;
import androidx.core.app.ActivityCompat;
import androidx.annotation.RequiresApi;
import java.util.Collections;
import java.util.Set;

public final class PairingActivity extends Activity {
    private static final int REQUEST_PERMISSIONS = 41;
    private static final int REQUEST_COMPANION = 42;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private BluetoothLeScanner scanner;
    private boolean completed;

    @Override
    protected void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        setTitle("Pair heart-rate sensor");
        ensurePermissionsThenPair();
    }

    private void ensurePermissionsThenPair() {
        final String[] permissions = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
            ? new String[] {Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT}
            : new String[] {Manifest.permission.ACCESS_FINE_LOCATION};
        for (String permission : permissions) {
            if (ActivityCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED) {
                ActivityCompat.requestPermissions(this, permissions, REQUEST_PERMISSIONS);
                return;
            }
        }
        beginPairing();
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(requestCode, permissions, results);
        if (requestCode != REQUEST_PERMISSIONS) return;
        for (int result : results) {
            if (result != PackageManager.PERMISSION_GRANTED) {
                fail("Bluetooth permission is required to pair a heart-rate sensor.");
                return;
            }
        }
        beginPairing();
    }

    @SuppressLint("MissingPermission")
    private void beginPairing() {
        final BluetoothManager bluetoothManager = getSystemService(BluetoothManager.class);
        final BluetoothAdapter adapter = bluetoothManager == null ? null : bluetoothManager.getAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            fail("Bluetooth is unavailable or disabled.");
            return;
        }
        final BluetoothDevice bonded = matchingBondedDevice(adapter.getBondedDevices());
        if (bonded != null) {
            saveDevice(bonded);
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            associateCompanion();
        } else {
            scanForDevice(adapter);
        }
    }

    @SuppressLint("MissingPermission")
    private BluetoothDevice matchingBondedDevice(Set<BluetoothDevice> devices) {
        for (BluetoothDevice device : devices) {
            final ParcelUuid[] uuids = device.getUuids();
            if (uuids == null) continue;
            for (ParcelUuid uuid : uuids) {
                if (HeartRatePluginService.HEART_RATE_SERVICE.equals(uuid.getUuid())) return device;
            }
        }
        return null;
    }

    @SuppressWarnings("deprecation")
    @RequiresApi(Build.VERSION_CODES.O)
    private void associateCompanion() {
        final CompanionDeviceManager manager = getSystemService(CompanionDeviceManager.class);
        if (manager == null) {
            fail("Companion Device Manager is unavailable.");
            return;
        }
        final ScanFilter scanFilter = new ScanFilter.Builder()
            .setServiceUuid(new ParcelUuid(HeartRatePluginService.HEART_RATE_SERVICE))
            .build();
        final BluetoothLeDeviceFilter deviceFilter = new BluetoothLeDeviceFilter.Builder()
            .setScanFilter(scanFilter)
            .build();
        final AssociationRequest request = new AssociationRequest.Builder()
            .addDeviceFilter(deviceFilter)
            .setSingleDevice(true)
            .build();
        manager.associate(request, new CompanionDeviceManager.Callback() {
            @Override
            public void onDeviceFound(IntentSender chooserLauncher) {
                launchChooser(chooserLauncher);
            }

            @Override
            public void onAssociationPending(IntentSender chooserLauncher) {
                launchChooser(chooserLauncher);
            }

            @Override
            public void onAssociationCreated(android.companion.AssociationInfo associationInfo) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    final String address = associationInfo.getDeviceMacAddress() == null
                        ? ""
                        : associationInfo.getDeviceMacAddress().toString();
                    if (!address.isEmpty()) saveAddress(address, "Heart-rate sensor");
                }
            }

            @Override
            public void onFailure(CharSequence error) {
                fail(error == null ? "No compatible heart-rate sensor was selected." : error.toString());
            }
        }, null);
    }

    private void launchChooser(IntentSender chooserLauncher) {
        try {
            startIntentSenderForResult(chooserLauncher, REQUEST_COMPANION, null, 0, 0, 0);
        } catch (IntentSender.SendIntentException error) {
            fail("Unable to open the companion-device chooser.");
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, @Nullable Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_COMPANION) return;
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        if (resultCode != RESULT_OK || data == null) {
            fail("No heart-rate sensor was selected.");
            return;
        }
        final Object selected = data.getParcelableExtra(CompanionDeviceManager.EXTRA_DEVICE);
        if (selected instanceof BluetoothDevice) {
            saveDevice((BluetoothDevice) selected);
        } else if (selected instanceof ScanResult) {
            saveDevice(((ScanResult) selected).getDevice());
        } else {
            fail("Android did not return a Bluetooth device.");
        }
    }

    @SuppressLint("MissingPermission")
    private void scanForDevice(BluetoothAdapter adapter) {
        scanner = adapter.getBluetoothLeScanner();
        if (scanner == null) {
            fail("BLE scanning is unavailable.");
            return;
        }
        final ScanFilter filter = new ScanFilter.Builder()
            .setServiceUuid(new ParcelUuid(HeartRatePluginService.HEART_RATE_SERVICE))
            .build();
        scanner.startScan(
            Collections.singletonList(filter),
            new ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(),
            scanCallback
        );
        handler.postDelayed(() -> {
            stopScan();
            if (!completed) fail("No BLE heart-rate sensor was found.");
        }, 15_000L);
    }

    private final ScanCallback scanCallback = new ScanCallback() {
        @Override
        public void onScanResult(int callbackType, ScanResult result) {
            saveDevice(result.getDevice());
        }

        @Override
        public void onScanFailed(int errorCode) {
            fail("BLE scan failed (" + errorCode + ").");
        }
    };

    @SuppressLint("MissingPermission")
    private void saveDevice(BluetoothDevice device) {
        saveAddress(device.getAddress(), device.getName() == null ? "Heart-rate sensor" : device.getName());
    }

    private void saveAddress(String address, String name) {
        if (completed) return;
        completed = true;
        stopScan();
        new PluginPreferences(this).setDevice(address, name);
        HeartRatePluginService.requestReconnect();
        setResult(RESULT_OK);
        finish();
    }

    @SuppressLint("MissingPermission")
    private void stopScan() {
        handler.removeCallbacksAndMessages(null);
        if (scanner != null) {
            try { scanner.stopScan(scanCallback); } catch (Exception ignored) {}
            scanner = null;
        }
    }

    private void fail(String message) {
        if (isFinishing()) return;
        stopScan();
        new AlertDialog.Builder(this)
            .setTitle("Pairing unavailable")
            .setMessage(message)
            .setPositiveButton(android.R.string.ok, (dialog, which) -> finish())
            .setOnCancelListener(dialog -> finish())
            .show();
    }

    @Override
    protected void onDestroy() {
        stopScan();
        super.onDestroy();
    }
}
