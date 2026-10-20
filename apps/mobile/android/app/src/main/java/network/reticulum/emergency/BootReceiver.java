package network.reticulum.emergency;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Build;

import androidx.core.content.ContextCompat;

public final class BootReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        if (context == null || intent == null) {
            return;
        }
        if (!Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())) {
            return;
        }
        final SharedPreferences preferences = context.getSharedPreferences(
            ReticulumNodeService.PREFS_NAME,
            Context.MODE_PRIVATE
        );
        // A foreground start must promote within seconds, so skip it when nothing will be restored.
        if (!NodeRuntimeLifecycleController.shouldRestoreAfterBoot(preferences)) {
            return;
        }
        final Intent serviceIntent = new Intent(context, ReticulumNodeService.class);
        serviceIntent.setAction(ReticulumNodeService.ACTION_RESTORE_AFTER_BOOT);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ContextCompat.startForegroundService(context, serviceIntent);
        } else {
            context.startService(serviceIntent);
        }
    }
}
