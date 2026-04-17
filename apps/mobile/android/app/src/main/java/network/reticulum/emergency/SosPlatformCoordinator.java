package network.reticulum.emergency;

import android.Manifest;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.BatteryManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Looper;
import android.util.Log;

import androidx.core.content.ContextCompat;

import org.json.JSONException;
import org.json.JSONObject;

final class SosPlatformCoordinator implements SensorEventListener, LocationListener {
    private static final String TAG = "SosPlatformCoordinator";
    static final long RECENT_LOCATION_MAX_AGE_MS = 5L * 60L * 1000L;
    static final int LOCATION_SOURCE_NONE = 0;
    static final int LOCATION_SOURCE_GPS = 1;
    static final int LOCATION_SOURCE_NETWORK = 2;

    private final ReticulumNodeService service;
    private final SensorManager sensorManager;
    private final LocationManager locationManager;
    private final Sensor accelerometer;
    private final BroadcastReceiver screenReceiver = new BroadcastReceiver() {
        @Override
        public void onReceive(Context context, Intent intent) {
            if (intent == null) {
                return;
            }
            final String action = intent.getAction();
            if (Intent.ACTION_SCREEN_ON.equals(action) || Intent.ACTION_SCREEN_OFF.equals(action)) {
                final JSONObject payload = new JSONObject();
                try {
                    submitTelemetrySnapshotIfStale(1000L);
                    payload.put("atMs", System.currentTimeMillis());
                    service.submitSosScreenEventJson(payload.toString());
                } catch (JSONException ex) {
                    Log.w(TAG, "Failed to forward SOS screen event", ex);
                }
            }
        }
    };

    private boolean accelerometerRegistered = false;
    private boolean screenRegistered = false;
    private boolean locationRegistered = false;
    private Location latestGpsLocation = null;
    private Location latestNetworkLocation = null;
    private long lastTelemetrySnapshotAtMs = 0L;

    SosPlatformCoordinator(ReticulumNodeService service) {
        this.service = service;
        sensorManager = (SensorManager) service.getSystemService(Context.SENSOR_SERVICE);
        locationManager = (LocationManager) service.getSystemService(Context.LOCATION_SERVICE);
        accelerometer = sensorManager == null ? null : sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
    }

    void applySettingsJson(String settingsJson) {
        try {
            final JSONObject settings = new JSONObject(settingsJson == null ? "{}" : settingsJson);
            final boolean enabled = settings.optBoolean("enabled", false);
            final boolean accelerometerNeeded = enabled
                && (settings.optBoolean("triggerShake", false) || settings.optBoolean("triggerTapPattern", false));
            final boolean powerNeeded = enabled && settings.optBoolean("triggerPowerButton", false);
            final boolean locationNeeded = enabled && settings.optBoolean("includeLocation", true);
            setAccelerometerEnabled(accelerometerNeeded);
            setScreenReceiverEnabled(powerNeeded);
            setLocationEnabled(locationNeeded);
            if (enabled) {
                submitTelemetrySnapshot();
            }
        } catch (JSONException ex) {
            Log.w(TAG, "Failed to apply SOS settings", ex);
            setAccelerometerEnabled(false);
            setScreenReceiverEnabled(false);
            setLocationEnabled(false);
        }
    }

    void submitTelemetrySnapshot() {
        submitTelemetrySnapshot(System.currentTimeMillis());
    }

    void submitTelemetrySnapshotIfStale(long maxAgeMs) {
        final long nowMs = System.currentTimeMillis();
        if (lastTelemetrySnapshotAtMs > 0 && nowMs - lastTelemetrySnapshotAtMs < maxAgeMs) {
            return;
        }
        submitTelemetrySnapshot(nowMs);
    }

    private void submitTelemetrySnapshot(long nowMs) {
        final JSONObject payload = new JSONObject();
        try {
            lastTelemetrySnapshotAtMs = nowMs;
            payload.put("updatedAtMs", nowMs);
            appendBattery(payload);
            appendLocation(payload, nowMs);
            service.submitSosTelemetryJson(payload.toString());
        } catch (JSONException ex) {
            Log.w(TAG, "Failed to forward SOS telemetry snapshot", ex);
        }
    }

    void close() {
        setAccelerometerEnabled(false);
        setScreenReceiverEnabled(false);
        setLocationEnabled(false);
    }

    @Override
    public void onSensorChanged(SensorEvent event) {
        if (event == null || event.values == null || event.values.length < 3) {
            return;
        }
        final JSONObject payload = new JSONObject();
        try {
            submitTelemetrySnapshotIfStale(10_000L);
            payload.put("x", event.values[0]);
            payload.put("y", event.values[1]);
            payload.put("z", event.values[2]);
            payload.put("atMs", System.currentTimeMillis());
            service.submitSosAccelerometerJson(payload.toString());
        } catch (JSONException ex) {
            Log.w(TAG, "Failed to forward SOS accelerometer sample", ex);
        }
    }

    @Override
    public void onAccuracyChanged(Sensor sensor, int accuracy) {
    }

    @Override
    public void onLocationChanged(Location location) {
        cacheLocation(location);
        submitTelemetrySnapshotIfStale(1000L);
    }

    @Override
    public void onProviderEnabled(String provider) {
    }

    @Override
    public void onProviderDisabled(String provider) {
    }

    @Override
    public void onStatusChanged(String provider, int status, Bundle extras) {
    }

    private void setAccelerometerEnabled(boolean enabled) {
        if (enabled == accelerometerRegistered) {
            return;
        }
        if (enabled) {
            if (sensorManager != null && accelerometer != null) {
                accelerometerRegistered = sensorManager.registerListener(
                    this,
                    accelerometer,
                    SensorManager.SENSOR_DELAY_GAME
                );
            }
            return;
        }
        if (sensorManager != null) {
            sensorManager.unregisterListener(this);
        }
        accelerometerRegistered = false;
    }

    private void setScreenReceiverEnabled(boolean enabled) {
        if (enabled == screenRegistered) {
            return;
        }
        if (enabled) {
            final IntentFilter filter = new IntentFilter();
            filter.addAction(Intent.ACTION_SCREEN_ON);
            filter.addAction(Intent.ACTION_SCREEN_OFF);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                service.registerReceiver(screenReceiver, filter, Context.RECEIVER_NOT_EXPORTED);
            } else {
                service.registerReceiver(screenReceiver, filter);
            }
            screenRegistered = true;
            return;
        }
        try {
            service.unregisterReceiver(screenReceiver);
        } catch (IllegalArgumentException ignored) {
        }
        screenRegistered = false;
    }

    private void setLocationEnabled(boolean enabled) {
        if (enabled == locationRegistered) {
            return;
        }
        if (!enabled) {
            if (locationManager != null) {
                try {
                    locationManager.removeUpdates(this);
                } catch (SecurityException ex) {
                    Log.w(TAG, "SOS location permission disappeared while removing updates", ex);
                }
            }
            locationRegistered = false;
            return;
        }
        if (locationManager == null || !hasAnyLocationPermission()) {
            locationRegistered = false;
            return;
        }
        boolean registered = false;
        try {
            if (hasFineLocationPermission()) {
                locationManager.requestLocationUpdates(
                    LocationManager.GPS_PROVIDER,
                    10_000L,
                    0.0f,
                    this,
                    Looper.getMainLooper()
                );
                registered = true;
            }
            locationManager.requestLocationUpdates(
                LocationManager.NETWORK_PROVIDER,
                10_000L,
                0.0f,
                this,
                Looper.getMainLooper()
            );
            registered = true;
        } catch (IllegalArgumentException ex) {
            Log.w(TAG, "SOS location provider unavailable", ex);
        } catch (SecurityException ex) {
            Log.w(TAG, "SOS location permission disappeared", ex);
        }
        locationRegistered = registered;
    }

    private void appendBattery(JSONObject payload) throws JSONException {
        final Intent battery = service.registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
        if (battery == null) {
            return;
        }
        final int level = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
        final int scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
        if (level >= 0 && scale > 0) {
            payload.put("batteryPercent", ((double) level / (double) scale) * 100.0);
        }
        final int status = battery.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
        payload.put(
            "batteryCharging",
            status == BatteryManager.BATTERY_STATUS_CHARGING || status == BatteryManager.BATTERY_STATUS_FULL
        );
    }

    private void appendLocation(JSONObject payload, long nowMs) throws JSONException {
        if (locationManager == null) {
            return;
        }
        final boolean fine = hasFineLocationPermission();
        final boolean coarse = hasCoarseLocationPermission();
        if (!fine && !coarse) {
            return;
        }
        try {
            final Location gps = fine
                ? newestLocation(latestGpsLocation, locationManager.getLastKnownLocation(LocationManager.GPS_PROVIDER))
                : null;
            final Location network = newestLocation(
                latestNetworkLocation,
                locationManager.getLastKnownLocation(LocationManager.NETWORK_PROVIDER)
            );
            final Location location = selectRecentLocation(gps, network, nowMs);
            if (location == null) {
                return;
            }
            payload.put("lat", location.getLatitude());
            payload.put("lon", location.getLongitude());
            if (location.hasAltitude()) {
                payload.put("alt", location.getAltitude());
            }
            if (location.hasSpeed()) {
                payload.put("speed", location.getSpeed());
            }
            if (location.hasBearing()) {
                payload.put("course", location.getBearing());
            }
            if (location.hasAccuracy()) {
                payload.put("accuracy", location.getAccuracy());
            }
        } catch (SecurityException ex) {
            Log.w(TAG, "SOS location permission disappeared", ex);
        }
    }

    private boolean hasFineLocationPermission() {
        return ContextCompat.checkSelfPermission(service, Manifest.permission.ACCESS_FINE_LOCATION)
            == PackageManager.PERMISSION_GRANTED;
    }

    private boolean hasCoarseLocationPermission() {
        return ContextCompat.checkSelfPermission(service, Manifest.permission.ACCESS_COARSE_LOCATION)
            == PackageManager.PERMISSION_GRANTED;
    }

    private boolean hasAnyLocationPermission() {
        return hasFineLocationPermission() || hasCoarseLocationPermission();
    }

    private void cacheLocation(Location location) {
        if (location == null) {
            return;
        }
        final String provider = location.getProvider();
        if (LocationManager.GPS_PROVIDER.equals(provider)) {
            latestGpsLocation = new Location(location);
            return;
        }
        if (LocationManager.NETWORK_PROVIDER.equals(provider)) {
            latestNetworkLocation = new Location(location);
        }
    }

    private Location selectRecentLocation(Location gps, Location network, long nowMs) {
        final int source = selectRecentLocationSource(
            gps != null,
            gps == null ? 0L : gps.getTime(),
            network != null,
            network == null ? 0L : network.getTime(),
            nowMs
        );
        if (source == LOCATION_SOURCE_GPS) {
            return gps;
        }
        if (source == LOCATION_SOURCE_NETWORK) {
            return network;
        }
        return null;
    }

    static Location newestLocation(Location first, Location second) {
        if (first == null) {
            return second;
        }
        if (second == null) {
            return first;
        }
        return newerLocationTime(first.getTime(), second.getTime()) == first.getTime() ? first : second;
    }

    static long newerLocationTime(long firstTimeMs, long secondTimeMs) {
        if (firstTimeMs <= 0L) {
            return secondTimeMs;
        }
        if (secondTimeMs <= 0L) {
            return firstTimeMs;
        }
        return Math.max(firstTimeMs, secondTimeMs);
    }

    static boolean isRecentLocationTime(long locationTimeMs, long nowMs) {
        return locationTimeMs > 0
            && locationTimeMs <= nowMs
            && nowMs - locationTimeMs <= RECENT_LOCATION_MAX_AGE_MS;
    }

    static int selectRecentLocationSource(
        boolean hasGps,
        long gpsTimeMs,
        boolean hasNetwork,
        long networkTimeMs,
        long nowMs
    ) {
        if (hasGps && isRecentLocationTime(gpsTimeMs, nowMs)) {
            return LOCATION_SOURCE_GPS;
        }
        if (hasNetwork && isRecentLocationTime(networkTimeMs, nowMs)) {
            return LOCATION_SOURCE_NETWORK;
        }
        return LOCATION_SOURCE_NONE;
    }
}
