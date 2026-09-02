package network.reticulum.emergency;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;

import androidx.core.content.ContextCompat;

final class BatteryPowerCoordinator implements AutoCloseable {
    interface BatteryListener {
        void onBatteryChanged(int level, int scale, boolean charging);
    }

    interface BatteryObservation extends AutoCloseable {
        void start(BatteryListener listener);

        @Override
        void close();
    }

    interface NativePowerSink {
        int updateBatteryState(int percent, boolean charging);
    }

    private final BatteryObservation observation;
    private final NativePowerSink nativePowerSink;
    private boolean started;

    BatteryPowerCoordinator(BatteryObservation observation, NativePowerSink nativePowerSink) {
        this.observation = observation;
        this.nativePowerSink = nativePowerSink;
    }

    static BatteryPowerCoordinator forContext(Context context, NativePowerSink nativePowerSink) {
        return new BatteryPowerCoordinator(
            new AndroidBatteryObservation(context.getApplicationContext()),
            nativePowerSink
        );
    }

    synchronized void start() {
        if (started) {
            return;
        }
        observation.start(this::forwardBatteryState);
        started = true;
    }

    private void forwardBatteryState(int level, int scale, boolean charging) {
        final int percent = batteryPercent(level, scale);
        if (percent >= 0) {
            nativePowerSink.updateBatteryState(percent, charging);
        }
    }

    static int batteryPercent(int level, int scale) {
        if (level < 0 || scale <= 0) {
            return -1;
        }
        return Math.max(0, Math.min(100, (level * 100) / scale));
    }

    @Override
    public synchronized void close() {
        if (!started) {
            return;
        }
        observation.close();
        started = false;
    }

    private static final class AndroidBatteryObservation implements BatteryObservation {
        private final Context context;
        private BroadcastReceiver receiver;

        AndroidBatteryObservation(Context context) {
            this.context = context;
        }

        @Override
        public void start(BatteryListener listener) {
            if (receiver != null) {
                return;
            }
            receiver = new BroadcastReceiver() {
                @Override
                public void onReceive(Context ignored, Intent intent) {
                    deliver(intent, listener);
                }
            };
            ContextCompat.registerReceiver(
                context,
                receiver,
                new IntentFilter(Intent.ACTION_BATTERY_CHANGED),
                ContextCompat.RECEIVER_NOT_EXPORTED
            );
        }

        private static void deliver(Intent intent, BatteryListener listener) {
            final int status = intent.getIntExtra(
                BatteryManager.EXTRA_STATUS,
                BatteryManager.BATTERY_STATUS_UNKNOWN
            );
            listener.onBatteryChanged(
                intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1),
                intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1),
                status == BatteryManager.BATTERY_STATUS_CHARGING
                    || status == BatteryManager.BATTERY_STATUS_FULL
            );
        }

        @Override
        public void close() {
            if (receiver == null) {
                return;
            }
            context.unregisterReceiver(receiver);
            receiver = null;
        }
    }
}
