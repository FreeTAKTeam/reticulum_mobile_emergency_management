package network.reticulum.emergency;

import android.Manifest;
import android.content.Context;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.Build;
import android.os.Bundle;
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
import com.getcapacitor.annotation.PermissionCallback;

import java.util.ArrayList;
import java.util.List;

@CapacitorPlugin(
    name = "TelemetryLocation",
    permissions = {
        @Permission(
            strings = { Manifest.permission.ACCESS_COARSE_LOCATION, Manifest.permission.ACCESS_FINE_LOCATION },
            alias = TelemetryLocationPlugin.LOCATION_ALIAS
        ),
        @Permission(
            strings = { Manifest.permission.ACCESS_COARSE_LOCATION },
            alias = TelemetryLocationPlugin.COARSE_LOCATION_ALIAS
        )
    }
)
public class TelemetryLocationPlugin extends Plugin {
    static final String LOCATION_ALIAS = "location";
    static final String COARSE_LOCATION_ALIAS = "coarseLocation";
    private static final String FUSED_PROVIDER = "fused";
    private static final long DEFAULT_TIMEOUT_MS = 15_000L;
    private static final long DEFAULT_MAXIMUM_AGE_MS = 5_000L;
    private static final long FALLBACK_MAXIMUM_AGE_MS = 10L * 60L * 1000L;

    @PluginMethod
    @Override
    public void checkPermissions(PluginCall call) {
        super.checkPermissions(call);
    }

    @PluginMethod
    @Override
    public void requestPermissions(PluginCall call) {
        super.requestPermissions(call);
    }

    @PluginMethod
    public void getCurrentPosition(PluginCall call) {
        handlePermissionRequest(call, "completeCurrentPosition", () -> resolveCurrentPosition(call));
    }

    private void handlePermissionRequest(
        PluginCall call,
        String callbackName,
        Runnable onPermissionGranted
    ) {
        final String alias = getAlias(call);
        if (getPermissionState(alias) != PermissionState.GRANTED) {
            requestPermissionForAlias(alias, call, callbackName);
            return;
        }
        onPermissionGranted.run();
    }

    @PermissionCallback
    private void completeCurrentPosition(PluginCall call) {
        if (
            getPermissionState(LOCATION_ALIAS) == PermissionState.GRANTED ||
            getPermissionState(COARSE_LOCATION_ALIAS) == PermissionState.GRANTED
        ) {
            resolveCurrentPosition(call);
            return;
        }
        call.reject("Location permission denied.");
    }

    private void resolveCurrentPosition(PluginCall call) {
        final LocationManager locationManager = (LocationManager) getContext().getSystemService(Context.LOCATION_SERVICE);
        if (locationManager == null) {
            call.reject("Unable to access device location.");
            return;
        }

        final boolean enableHighAccuracy = Boolean.TRUE.equals(call.getBoolean("enableHighAccuracy", true));
        final long timeoutMs = normalizePositiveLong(call.getLong("timeout"), DEFAULT_TIMEOUT_MS);
        final long maximumAgeMs = normalizeNonNegativeLong(call.getLong("maximumAge"), DEFAULT_MAXIMUM_AGE_MS);

        final List<String> providers = selectProviders(locationManager, enableHighAccuracy);
        if (providers.isEmpty()) {
            call.reject("Location services are not enabled.");
            return;
        }

        final Location cachedLocation = findBestLastKnownLocation(locationManager, providers, maximumAgeMs);
        if (cachedLocation != null) {
            call.resolve(toPosition(cachedLocation));
            return;
        }

        final Location fallbackLocation = findBestLastKnownLocation(
            locationManager,
            providers,
            Math.max(maximumAgeMs, FALLBACK_MAXIMUM_AGE_MS)
        );

        new SingleFixRequest(call, locationManager, providers, timeoutMs, fallbackLocation).start();
    }

    private String getAlias(PluginCall call) {
        final boolean enableHighAccuracy = Boolean.TRUE.equals(call.getBoolean("enableHighAccuracy", true));
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && !enableHighAccuracy) {
            return COARSE_LOCATION_ALIAS;
        }
        return LOCATION_ALIAS;
    }

    private static long normalizePositiveLong(Long value, long fallback) {
        if (value == null || value <= 0L) {
            return fallback;
        }
        return value;
    }

    private static long normalizeNonNegativeLong(Long value, long fallback) {
        if (value == null || value < 0L) {
            return fallback;
        }
        return value;
    }

    private List<String> selectProviders(LocationManager locationManager, boolean enableHighAccuracy) {
        final List<String> providers = new ArrayList<>();
        final boolean gpsEnabled = locationManager.isProviderEnabled(LocationManager.GPS_PROVIDER);
        final boolean networkEnabled = locationManager.isProviderEnabled(LocationManager.NETWORK_PROVIDER);
        final boolean fusedEnabled = isProviderEnabled(locationManager, FUSED_PROVIDER);

        if (enableHighAccuracy) {
            if (gpsEnabled) {
                providers.add(LocationManager.GPS_PROVIDER);
            }
            if (fusedEnabled) {
                providers.add(FUSED_PROVIDER);
            }
            if (networkEnabled) {
                providers.add(LocationManager.NETWORK_PROVIDER);
            }
        } else {
            if (fusedEnabled) {
                providers.add(FUSED_PROVIDER);
            }
            if (networkEnabled) {
                providers.add(LocationManager.NETWORK_PROVIDER);
            }
            if (!networkEnabled && gpsEnabled) {
                providers.add(LocationManager.GPS_PROVIDER);
            }
        }

        if (providers.isEmpty() && gpsEnabled) {
            providers.add(LocationManager.GPS_PROVIDER);
        }
        if (providers.isEmpty() && fusedEnabled) {
            providers.add(FUSED_PROVIDER);
        }
        if (providers.isEmpty() && networkEnabled) {
            providers.add(LocationManager.NETWORK_PROVIDER);
        }
        return providers;
    }

    private static boolean isProviderEnabled(LocationManager locationManager, String provider) {
        try {
            return locationManager.getAllProviders().contains(provider) && locationManager.isProviderEnabled(provider);
        } catch (Exception ex) {
            return false;
        }
    }

    private Location findBestLastKnownLocation(
        LocationManager locationManager,
        List<String> providers,
        long maximumAgeMs
    ) {
        final long now = System.currentTimeMillis();
        Location bestLocation = null;
        for (String provider : providers) {
            final Location location;
            try {
                location = locationManager.getLastKnownLocation(provider);
            } catch (SecurityException ex) {
                return null;
            }

            if (location == null) {
                continue;
            }

            if ((now - location.getTime()) > maximumAgeMs) {
                continue;
            }

            if (bestLocation == null || isBetterLocation(location, bestLocation)) {
                bestLocation = location;
            }
        }
        return bestLocation;
    }

    private boolean isBetterLocation(Location candidate, Location currentBest) {
        final float candidateAccuracy = candidate.hasAccuracy() ? candidate.getAccuracy() : Float.MAX_VALUE;
        final float currentAccuracy = currentBest.hasAccuracy() ? currentBest.getAccuracy() : Float.MAX_VALUE;
        if (candidateAccuracy != currentAccuracy) {
            return candidateAccuracy < currentAccuracy;
        }
        return candidate.getTime() > currentBest.getTime();
    }

    private JSObject toPosition(Location location) {
        final JSObject coords = new JSObject();
        coords.put("latitude", location.getLatitude());
        coords.put("longitude", location.getLongitude());
        coords.put("accuracy", location.hasAccuracy() ? location.getAccuracy() : 0d);
        coords.put("altitude", location.hasAltitude() ? location.getAltitude() : null);
        coords.put("heading", location.hasBearing() ? location.getBearing() : null);
        coords.put("speed", location.hasSpeed() ? location.getSpeed() : null);

        final JSObject position = new JSObject();
        position.put("coords", coords);
        position.put("timestamp", location.getTime());
        return position;
    }

    private final class SingleFixRequest implements LocationListener {
        private final PluginCall call;
        private final LocationManager locationManager;
        private final List<String> providers;
        private final long timeoutMs;
        private final Location fallbackLocation;
        private final Handler handler = new Handler(Looper.getMainLooper());
        private boolean finished = false;

        SingleFixRequest(
            PluginCall call,
            LocationManager locationManager,
            List<String> providers,
            long timeoutMs,
            Location fallbackLocation
        ) {
            this.call = call;
            this.locationManager = locationManager;
            this.providers = providers;
            this.timeoutMs = timeoutMs;
            this.fallbackLocation = fallbackLocation;
        }

        void start() {
            try {
                for (String provider : providers) {
                    locationManager.requestLocationUpdates(provider, 0L, 0f, this, Looper.getMainLooper());
                }
            } catch (SecurityException ex) {
                finishWithError("Location permission denied.");
                return;
            } catch (IllegalArgumentException ex) {
                finishWithError("Unable to access device location.");
                return;
            }

            handler.postDelayed(this::finishWithFallbackOrError, timeoutMs);
        }

        @Override
        public void onLocationChanged(@NonNull Location location) {
            finishWithSuccess(location);
        }

        @Override
        public void onProviderDisabled(@NonNull String provider) {
            if (!hasEnabledProvider()) {
                finishWithError("Location services are not enabled.");
            }
        }

        @Override
        public void onStatusChanged(String provider, int status, Bundle extras) {
            // Deprecated callback retained for older Android API compatibility.
        }

        private boolean hasEnabledProvider() {
            for (String provider : providers) {
                if (locationManager.isProviderEnabled(provider)) {
                    return true;
                }
            }
            return false;
        }

        private void finishWithSuccess(Location location) {
            if (finished) {
                return;
            }
            finished = true;
            cleanup();
            call.resolve(toPosition(location));
        }

        private void finishWithError(String message) {
            if (finished) {
                return;
            }
            finished = true;
            cleanup();
            call.reject(message);
        }

        private void finishWithFallbackOrError() {
            if (fallbackLocation != null) {
                finishWithSuccess(fallbackLocation);
                return;
            }
            finishWithError("Unable to read device location.");
        }

        private void cleanup() {
            handler.removeCallbacksAndMessages(null);
            try {
                locationManager.removeUpdates(this);
            } catch (Exception ignored) {
                // Best effort cleanup only.
            }
        }
    }
}
