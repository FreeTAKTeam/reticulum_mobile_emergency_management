package network.reticulum.emergency;

import android.Manifest;
import android.content.Context;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.Handler;
import android.os.Looper;

import androidx.annotation.NonNull;

import com.getcapacitor.JSObject;
import com.getcapacitor.PermissionState;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;

@CapacitorPlugin(
    name = "TelemetryLocation",
    permissions = {
        @Permission(
            alias = TelemetryLocationPlugin.LOCATION_ALIAS,
            strings = {
                Manifest.permission.ACCESS_COARSE_LOCATION,
                Manifest.permission.ACCESS_FINE_LOCATION
            }
        ),
        @Permission(
            alias = TelemetryLocationPlugin.COARSE_LOCATION_ALIAS,
            strings = { Manifest.permission.ACCESS_COARSE_LOCATION }
        )
    }
)
public final class TelemetryLocationPlugin extends Plugin implements LocationListener {
    static final String LOCATION_ALIAS = "location";
    static final String COARSE_LOCATION_ALIAS = "coarseLocation";

    private static final long DEFAULT_TIMEOUT_MS = 15_000L;
    private static final long DEFAULT_MAXIMUM_AGE_MS = 5_000L;

    private final Handler mainHandler = new Handler(Looper.getMainLooper());

    private PluginCall activeCall;
    private LocationManager locationManager;
    private Runnable timeoutRunnable;

    @Override
    public void load() {
        super.load();
        locationManager = (LocationManager) getContext().getSystemService(Context.LOCATION_SERVICE);
    }

    @PluginMethod
    @Override
    public void checkPermissions(PluginCall call) {
        final JSObject result = new JSObject();
        result.put(LOCATION_ALIAS, permissionStateFor(LOCATION_ALIAS));
        result.put(COARSE_LOCATION_ALIAS, permissionStateFor(COARSE_LOCATION_ALIAS));
        call.resolve(result);
    }

    @PluginMethod
    @Override
    public void requestPermissions(PluginCall call) {
        if (getPermissionState(LOCATION_ALIAS) == PermissionState.GRANTED) {
            checkPermissions(call);
            return;
        }
        requestPermissionForAlias(LOCATION_ALIAS, call, "permissionsCallback");
    }

    @PluginMethod
    public void getCurrentPosition(PluginCall call) {
        if (locationManager == null) {
            call.reject("Location unavailable.");
            return;
        }
        if (getPermissionState(LOCATION_ALIAS) != PermissionState.GRANTED) {
            call.reject("Location permission denied.");
            return;
        }

        final boolean enableHighAccuracy = call.getBoolean("enableHighAccuracy", true);
        final long timeoutMs = sanitizeDuration(call.getLong("timeout"), DEFAULT_TIMEOUT_MS);
        final long maximumAgeMs = sanitizeDuration(call.getLong("maximumAge"), DEFAULT_MAXIMUM_AGE_MS);
        final String provider = selectProvider(enableHighAccuracy);
        if (provider == null) {
            call.reject("Location unavailable.");
            return;
        }

        final Location cachedLocation = locationManager.getLastKnownLocation(provider);
        if (isFreshEnough(cachedLocation, maximumAgeMs)) {
            call.resolve(positionFromLocation(cachedLocation));
            return;
        }

        cancelActiveRequest("Location unavailable.");
        bridge.saveCall(call);
        activeCall = call;
        try {
            locationManager.requestLocationUpdates(provider, 0L, 0f, this, Looper.getMainLooper());
        } catch (SecurityException ex) {
            final PluginCall pending = takeActiveCall();
            if (pending != null) {
                pending.reject("Location permission denied.", ex);
            }
            return;
        } catch (IllegalArgumentException ex) {
            final PluginCall pending = takeActiveCall();
            if (pending != null) {
                pending.reject("Location unavailable.", ex);
            }
            return;
        }

        timeoutRunnable = () -> {
            final PluginCall pending = takeActiveCall();
            if (pending != null) {
                pending.reject("Location unavailable.");
            }
        };
        mainHandler.postDelayed(timeoutRunnable, timeoutMs);
    }

    @PluginMethod(returnType = PluginMethod.RETURN_NONE)
    public void permissionsCallback(PluginCall call) {
        checkPermissions(call);
    }

    @Override
    public void onLocationChanged(@NonNull Location location) {
        final PluginCall pending = takeActiveCall();
        if (pending != null) {
            pending.resolve(positionFromLocation(location));
        }
    }

    @Override
    public void onProviderDisabled(@NonNull String provider) {
        final PluginCall pending = takeActiveCall();
        if (pending != null) {
            pending.reject("Location unavailable.");
        }
    }

    @Override
    public void onProviderEnabled(@NonNull String provider) {
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onStatusChanged(String provider, int status, android.os.Bundle extras) {
    }

    @Override
    protected void handleOnDestroy() {
        cancelActiveRequest("Location unavailable.");
        super.handleOnDestroy();
    }

    private String permissionStateFor(String alias) {
        final PermissionState state = getPermissionState(alias);
        if (state == PermissionState.GRANTED) {
            return "granted";
        }
        if (state == PermissionState.PROMPT_WITH_RATIONALE) {
            return "prompt-with-rationale";
        }
        if (state == PermissionState.DENIED) {
            return "denied";
        }
        return "prompt";
    }

    private String selectProvider(boolean enableHighAccuracy) {
        final boolean gpsEnabled = isProviderEnabled(LocationManager.GPS_PROVIDER);
        final boolean networkEnabled = isProviderEnabled(LocationManager.NETWORK_PROVIDER);
        if (enableHighAccuracy) {
            if (gpsEnabled) {
                return LocationManager.GPS_PROVIDER;
            }
            if (networkEnabled) {
                return LocationManager.NETWORK_PROVIDER;
            }
            return null;
        }
        if (networkEnabled) {
            return LocationManager.NETWORK_PROVIDER;
        }
        if (gpsEnabled) {
            return LocationManager.GPS_PROVIDER;
        }
        return null;
    }

    private boolean isProviderEnabled(String provider) {
        if (locationManager == null) {
            return false;
        }
        try {
            return locationManager.isProviderEnabled(provider);
        } catch (Exception ignored) {
            return false;
        }
    }

    private boolean isFreshEnough(Location location, long maximumAgeMs) {
        if (location == null) {
            return false;
        }
        final long ageMs = System.currentTimeMillis() - location.getTime();
        return ageMs >= 0L && ageMs <= maximumAgeMs;
    }

    private JSObject positionFromLocation(Location location) {
        final JSObject coords = new JSObject();
        coords.put("latitude", location.getLatitude());
        coords.put("longitude", location.getLongitude());
        coords.put("accuracy", (double) location.getAccuracy());
        if (location.hasAltitude()) {
            coords.put("altitude", location.getAltitude());
        } else {
            coords.put("altitude", null);
        }
        if (location.hasBearing()) {
            coords.put("heading", (double) location.getBearing());
        } else {
            coords.put("heading", null);
        }
        if (location.hasSpeed()) {
            coords.put("speed", (double) location.getSpeed());
        } else {
            coords.put("speed", null);
        }

        final JSObject payload = new JSObject();
        payload.put("coords", coords);
        payload.put("timestamp", location.getTime());
        return payload;
    }

    private long sanitizeDuration(Long providedValue, long fallbackValue) {
        if (providedValue == null || providedValue <= 0L) {
            return fallbackValue;
        }
        return providedValue;
    }

    private void cancelActiveRequest(String message) {
        final PluginCall pending = takeActiveCall();
        if (pending != null) {
            pending.reject(message);
        }
    }

    private PluginCall takeActiveCall() {
        final PluginCall pending = activeCall;
        if (timeoutRunnable != null) {
            mainHandler.removeCallbacks(timeoutRunnable);
            timeoutRunnable = null;
        }
        if (locationManager != null) {
            try {
                locationManager.removeUpdates(this);
            } catch (SecurityException ignored) {
            }
        }
        activeCall = null;
        return pending;
    }
}
