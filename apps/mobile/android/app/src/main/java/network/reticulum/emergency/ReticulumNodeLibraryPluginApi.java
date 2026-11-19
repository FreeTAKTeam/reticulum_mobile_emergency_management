package network.reticulum.emergency;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.provider.OpenableColumns;
import android.util.Base64;
import androidx.activity.result.ActivityResult;
import androidx.core.content.FileProvider;
import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.ActivityCallback;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import org.json.JSONObject;

/**
 * Mission Library (REM #356). The Storage Access Framework picker hands us a document URI; we
 * copy its bytes once into a private cache file and pass the path to the native runtime, which
 * hashes, stores and indexes it. No document URI is kept and no file bytes reach the WebView,
 * except bounded image thumbnails.
 */
public abstract class ReticulumNodeLibraryPluginApi extends ReticulumNodeChecklistPluginApi {

    private static final int THUMBNAIL_MAX_EDGE = 256;
    private static final long PICK_MAX_BYTES = 64L * 1024L * 1024L;

    @PluginMethod
    public void listLibraryItems(PluginCall call) {
        final JSObject query = call.getObject("query", new JSObject());
        runStringServiceCall(call, "Failed to list the library.", "Invalid library response.",
            service -> service.listLibraryItemsJson(query.toString()));
    }

    @PluginMethod
    public void refreshLibraryHub(PluginCall call) {
        final JSObject query = call.getObject("query", new JSObject());
        runStringServiceCall(call, "Failed to ask the hub.", "Invalid library response.",
            service -> service.refreshLibraryHubJson(query.toString()));
    }

    @PluginMethod
    public void deleteLibraryItem(PluginCall call) {
        final JSObject payload = itemPayload(call);
        runStringServiceCall(call, "Failed to remove the item.", "Invalid library response.",
            service -> service.deleteLibraryItemJson(payload.toString()));
    }

    @PluginMethod
    public void setLibraryItemMissions(PluginCall call) {
        final JSObject payload = itemPayload(call);
        payload.put("missionUids", call.getArray("missionUids", new com.getcapacitor.JSArray()));
        runStringServiceCall(call, "Failed to update missions.", "Invalid library response.",
            service -> service.setLibraryItemMissionsJson(payload.toString()));
    }

    @PluginMethod
    public void shareLibraryItem(PluginCall call) {
        final JSObject payload = itemPayload(call);
        payload.put("destinationHex", call.getString("destinationHex", ""));
        runStringServiceCall(call, "Failed to share the item.", "Invalid library response.",
            service -> service.shareLibraryItemJson(payload.toString()));
    }

    @PluginMethod
    public void downloadLibraryItem(PluginCall call) {
        final JSObject payload = itemPayload(call);
        runStringServiceCall(call, "Failed to start the download.", "Invalid library response.",
            service -> service.downloadLibraryItemJson(payload.toString()));
    }

    @PluginMethod
    public void importLibraryFile(PluginCall call) {
        final Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        final String kind = call.getString("kind", "all");
        intent.setType("image".equals(kind) ? "image/*" : "*/*");
        startActivityForResult(call, intent, "onLibraryDocumentPicked");
    }

    @ActivityCallback
    private void onLibraryDocumentPicked(PluginCall call, ActivityResult result) {
        if (call == null) {
            return;
        }
        final Intent data = result.getData();
        if (result.getResultCode() != Activity.RESULT_OK || data == null || data.getData() == null) {
            final JSObject cancelled = new JSObject();
            cancelled.put("cancelled", true);
            call.resolve(cancelled);
            return;
        }
        final Uri uri = data.getData();
        executeBridgeTask(() -> importPicked(call, uri));
    }

    private void importPicked(PluginCall call, Uri uri) {
        final ContentResolver resolver = getContext().getContentResolver();
        final File temp = new File(getContext().getCacheDir(), "library-import-" + System.nanoTime());
        try {
            final String name = displayName(resolver, uri);
            final String mediaType = resolver.getType(uri);
            try (InputStream input = resolver.openInputStream(uri);
                 OutputStream output = new FileOutputStream(temp)) {
                if (input == null) {
                    call.reject("The selected document could not be opened.");
                    return;
                }
                final byte[] buffer = new byte[64 * 1024];
                long total = 0;
                int read;
                while ((read = input.read(buffer)) != -1) {
                    total += read;
                    if (total > PICK_MAX_BYTES) {
                        call.reject("Files are limited to 64 MiB.");
                        return;
                    }
                    output.write(buffer, 0, read);
                }
            }
            final JSObject payload = new JSObject();
            payload.put("path", temp.getAbsolutePath());
            payload.put("name", name);
            payload.put("mediaType", mediaType);
            final String raw = awaitService().importLibraryFileJson(payload.toString());
            if (raw == null || raw.isEmpty()) {
                rejectFromNative(call, "Failed to import the document.");
                return;
            }
            call.resolve(new JSObject(raw));
        } catch (Exception error) {
            // Includes running out of space while copying.
            call.reject("Failed to import the document.", error);
        } finally {
            //noinspection ResultOfMethodCallIgnored
            temp.delete();
        }
    }

    @PluginMethod
    public void openLibraryItem(PluginCall call) {
        final JSObject payload = itemPayload(call);
        executeBridgeTask(() -> {
            try {
                final JSONObject entry = libraryEntry(payload);
                if (entry == null || entry.isNull("path")) {
                    call.reject("This item is not stored on the device.");
                    return;
                }
                final JSONObject item = entry.getJSONObject("item");
                final File file = new File(entry.getString("path"));
                final Uri uri = FileProvider.getUriForFile(
                    getContext(), getContext().getPackageName() + ".fileprovider", file);
                final Intent view = new Intent(Intent.ACTION_VIEW);
                view.setDataAndType(uri, item.optString("media_type", "application/octet-stream"));
                view.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
                getActivity().runOnUiThread(() -> {
                    try {
                        getContext().startActivity(Intent.createChooser(view, item.optString("name", "Open"))
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                        call.resolve(new JSObject());
                    } catch (ActivityNotFoundException missing) {
                        call.reject("No app on this device can open this file.");
                    }
                });
            } catch (Exception error) {
                call.reject("Failed to open the item.", error);
            }
        });
    }

    /** A JPEG thumbnail no larger than {@value #THUMBNAIL_MAX_EDGE}px, as a data URL. */
    @PluginMethod
    public void getLibraryThumbnail(PluginCall call) {
        final JSObject payload = itemPayload(call);
        executeBridgeTask(() -> {
            try {
                final JSONObject entry = libraryEntry(payload);
                final JSObject out = new JSObject();
                out.put("dataUrl", entry == null || entry.isNull("path") ? null : thumbnail(entry.getString("path")));
                call.resolve(out);
            } catch (Exception error) {
                call.reject("Failed to read the thumbnail.", error);
            }
        });
    }

    private JSONObject libraryEntry(JSObject payload) throws Exception {
        final String raw = awaitService().getLibraryItemJson(payload.toString());
        return raw == null || raw.isEmpty() ? null : new JSONObject(raw);
    }

    private static JSObject itemPayload(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("itemId", call.getString("itemId", ""));
        return payload;
    }

    private static String thumbnail(String path) {
        final BitmapFactory.Options bounds = new BitmapFactory.Options();
        bounds.inJustDecodeBounds = true;
        BitmapFactory.decodeFile(path, bounds);
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
            return null;
        }
        int sample = 1;
        while (Math.max(bounds.outWidth, bounds.outHeight) / (sample * 2) >= THUMBNAIL_MAX_EDGE) {
            sample *= 2;
        }
        final BitmapFactory.Options decode = new BitmapFactory.Options();
        decode.inSampleSize = sample;
        final Bitmap decoded = BitmapFactory.decodeFile(path, decode);
        if (decoded == null) {
            return null;
        }
        final float scale = Math.min(1f, (float) THUMBNAIL_MAX_EDGE / Math.max(decoded.getWidth(), decoded.getHeight()));
        final Bitmap scaled = Bitmap.createScaledBitmap(decoded,
            Math.max(1, Math.round(decoded.getWidth() * scale)),
            Math.max(1, Math.round(decoded.getHeight() * scale)), true);
        final ByteArrayOutputStream jpeg = new ByteArrayOutputStream();
        scaled.compress(Bitmap.CompressFormat.JPEG, 70, jpeg);
        return "data:image/jpeg;base64," + Base64.encodeToString(jpeg.toByteArray(), Base64.NO_WRAP);
    }

    private static String displayName(ContentResolver resolver, Uri uri) {
        try (Cursor cursor = resolver.query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                final String name = cursor.getString(0);
                if (name != null && !name.trim().isEmpty()) {
                    return name;
                }
            }
        }
        return "document";
    }
}
