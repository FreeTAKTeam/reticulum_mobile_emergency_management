package network.reticulum.emergency.wearables;

import android.Manifest;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;

import androidx.core.content.ContextCompat;

import java.util.ArrayList;
import java.util.List;

public final class BlePermissionHelper {
    public static final String ERROR_MISSING_PERMISSIONS = "MissingWearablePermissions";
    public static final String ERROR_BLUETOOTH_UNAVAILABLE = "BluetoothUnavailable";
    public static final String ERROR_BLUETOOTH_DISABLED = "BluetoothDisabled";

    private BlePermissionHelper() {}

    public static String[] requiredRuntimePermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return new String[] {
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_CONNECT
            };
        }
        return new String[] { Manifest.permission.ACCESS_FINE_LOCATION };
    }

    public static boolean hasScanPermission(Context context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return hasPermission(context, Manifest.permission.BLUETOOTH_SCAN);
        }
        return hasPermission(context, Manifest.permission.ACCESS_FINE_LOCATION);
    }

    public static boolean hasConnectPermission(Context context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return hasPermission(context, Manifest.permission.BLUETOOTH_CONNECT);
        }
        return true;
    }

    public static List<String> missingRuntimePermissions(Context context) {
        final List<String> missing = new ArrayList<>();
        for (String permission : requiredRuntimePermissions()) {
            if (!hasPermission(context, permission)) {
                missing.add(permission);
            }
        }
        return missing;
    }

    public static boolean hasRequiredPermissions(Context context) {
        return missingRuntimePermissions(context).isEmpty();
    }

    private static boolean hasPermission(Context context, String permission) {
        return ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED;
    }
}
