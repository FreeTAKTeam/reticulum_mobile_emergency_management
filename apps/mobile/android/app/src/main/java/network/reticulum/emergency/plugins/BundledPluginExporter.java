package network.reticulum.emergency.plugins;

import android.Manifest;
import android.app.Activity;
import android.content.ContentValues;
import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;
import android.util.Log;
import android.widget.Toast;
import androidx.core.app.ActivityCompat;
import androidx.core.content.ContextCompat;
import java.io.File;
import java.io.FileOutputStream;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicBoolean;

/** Copies release-bundled installers to Downloads; installing them remains the user's choice. */
public final class BundledPluginExporter {
    private static final String TAG = "REM.PluginExport";
    private static final int STORAGE_REQUEST = 5217;
    private static final String ASSET_DIR = "rem-plugins";
    private static final String FOLDER = "REM Plugins";
    private static final AtomicBoolean EXPORTING = new AtomicBoolean();

    private BundledPluginExporter() {}

    public static void start(Activity activity) {
        try {
            String[] assets = activity.getAssets().list(ASSET_DIR);
            if (assets == null || assets.length == 0) return;
            if (Build.VERSION.SDK_INT < 29 && ContextCompat.checkSelfPermission(activity,
                    Manifest.permission.WRITE_EXTERNAL_STORAGE) != PackageManager.PERMISSION_GRANTED) {
                ActivityCompat.requestPermissions(activity,
                    new String[] { Manifest.permission.WRITE_EXTERNAL_STORAGE }, STORAGE_REQUEST);
                return;
            }
            exportAsync(activity, assets);
        } catch (IOException error) {
            report(activity, error);
        }
    }

    public static void onPermissionResult(Activity activity, int requestCode, int[] results) {
        if (requestCode != STORAGE_REQUEST) return;
        if (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) {
            start(activity);
        } else {
            Toast.makeText(activity, "Storage access is needed to save bundled plugin installers.", Toast.LENGTH_LONG).show();
        }
    }

    private static void exportAsync(Activity activity, String[] assets) {
        if (!EXPORTING.compareAndSet(false, true)) return;
        Context context = activity.getApplicationContext();
        new Thread(() -> {
            try {
                validateAssets(assets);
                for (String name : assets) {
                    export(context, name);
                }
            } catch (IOException | RuntimeException error) {
                report(activity, error);
            } finally {
                EXPORTING.set(false);
            }
        }, "rem-plugin-export").start();
    }

    private static void export(Context context, String name) throws IOException {
        SharedPreferences preferences = context.getSharedPreferences("bundled-plugins", Context.MODE_PRIVATE);
        String previous = preferences.getString(name, "");
        if (!previous.isEmpty()) {
            try (InputStream ignored = context.getContentResolver().openInputStream(Uri.parse(previous))) {
                if (ignored != null) {
                    verifyExisting(ignored, name);
                    return;
                }
            } catch (IOException | SecurityException ignored) {
                // A removed file can be exported again.
            }
        }
        Uri outputUri = null;
        File file = null;
        File destination = null;
        try {
            OutputStream output;
            if (Build.VERSION.SDK_INT >= 29) {
                ContentValues values = new ContentValues();
                values.put(MediaStore.Downloads.DISPLAY_NAME, name);
                values.put(MediaStore.Downloads.MIME_TYPE, "application/vnd.android.package-archive");
                values.put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/" + FOLDER);
                values.put(MediaStore.Downloads.IS_PENDING, 1);
                outputUri = context.getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (outputUri == null) throw new IOException("Could not create plugin installer");
                output = context.getContentResolver().openOutputStream(outputUri);
                if (output == null) throw new IOException("Could not open plugin installer");
            } else {
                File directory = new File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS), FOLDER);
                if (!directory.isDirectory() && !directory.mkdirs()) throw new IOException("Could not create plugin folder");
                destination = new File(directory, name);
                if (destination.exists()) {
                    try (InputStream existing = new FileInputStream(destination)) {
                        verifyExisting(existing, name);
                    }
                    return;
                }
                file = File.createTempFile("rem-plugin-", ".partial", directory);
                output = new FileOutputStream(file);
            }
            try (InputStream input = context.getAssets().open(ASSET_DIR + "/" + name); OutputStream target = output) {
                copyVerified(input, target, name);
            }
            if (outputUri != null) {
                ContentValues done = new ContentValues();
                done.put(MediaStore.Downloads.IS_PENDING, 0);
                if (context.getContentResolver().update(outputUri, done, null, null) != 1) {
                    throw new IOException("Could not finish plugin installer");
                }
                preferences.edit().putString(name, outputUri.toString()).apply();
            } else {
                Files.move(file.toPath(), destination.toPath());
            }
        } catch (IOException | RuntimeException error) {
            if (outputUri != null) context.getContentResolver().delete(outputUri, null, null);
            if (file != null && file.exists() && !file.delete()) Log.w(TAG, "Could not remove incomplete installer");
            throw error;
        }
    }

    static void validateAssets(String[] assets) throws IOException {
        String[] names = { "ble-heart-rate", "watch-status", "atn-rangefinder" };
        if (assets.length != names.length) throw new IOException("Expected three plugin installers");
        for (String name : names) {
            if (Arrays.stream(assets).filter(asset -> asset.matches(name + "-[a-f0-9]{64}\\.apk")).count() != 1) {
                throw new IOException("Missing or invalid plugin installer: " + name);
            }
        }
    }

    static void copyVerified(InputStream input, OutputStream output, String name) throws IOException {
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            byte[] buffer = new byte[16384];
            int count;
            while ((count = input.read(buffer)) != -1) {
                digest.update(buffer, 0, count);
                output.write(buffer, 0, count);
            }
            StringBuilder actual = new StringBuilder();
            for (byte value : digest.digest()) actual.append(String.format(java.util.Locale.ROOT, "%02x", value & 0xff));
            if (!name.endsWith("-" + actual + ".apk")) throw new IOException("Plugin installer checksum mismatch");
        } catch (NoSuchAlgorithmException error) {
            throw new IOException("SHA-256 unavailable", error);
        }
    }

    private static void verifyExisting(InputStream input, String name) throws IOException {
        copyVerified(input, new OutputStream() {
            @Override public void write(int value) {}
            @Override public void write(byte[] data, int offset, int length) {}
        }, name);
    }

    private static void report(Activity activity, Exception error) {
        Log.e(TAG, "Could not save bundled plugin installers", error);
        activity.runOnUiThread(() -> Toast.makeText(activity,
            "Could not save plugin installers. Open REM again to retry.", Toast.LENGTH_LONG).show());
    }
}
