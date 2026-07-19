package network.reticulum.emergency.plugin.api;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.os.Build;
import java.security.MessageDigest;
import java.util.Collections;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;

public final class CallerCertificateVerifier {
    private CallerCertificateVerifier() {}

    public static boolean isAllowed(Context context, int uid, Set<String> allowedFingerprints) {
        return isAllowed(context, uid, Collections.emptySet(), allowedFingerprints);
    }

    public static boolean isAllowed(
        Context context,
        int uid,
        Set<String> allowedPackageNames,
        Set<String> allowedFingerprints
    ) {
        if (context == null || allowedFingerprints == null || allowedFingerprints.isEmpty()) {
            return false;
        }
        final Set<String> normalized = new HashSet<>();
        for (String fingerprint : allowedFingerprints) {
            final String value = normalizeFingerprint(fingerprint);
            if (!value.isEmpty()) {
                normalized.add(value);
            }
        }
        final String[] packages = context.getPackageManager().getPackagesForUid(uid);
        if (packages == null) {
            return false;
        }
        for (String packageName : packages) {
            if (allowedPackageNames != null
                && !allowedPackageNames.isEmpty()
                && !allowedPackageNames.contains(packageName)) {
                continue;
            }
            for (String fingerprint : packageFingerprints(context, packageName)) {
                if (normalized.contains(fingerprint)) {
                    return true;
                }
            }
        }
        return false;
    }

    public static Set<String> packageFingerprints(Context context, String packageName) {
        final Set<String> fingerprints = new HashSet<>(
            currentPackageFingerprints(context, packageName)
        );
        fingerprints.addAll(packageCertificateHistory(context, packageName));
        return fingerprints;
    }

    public static Set<String> currentPackageFingerprints(Context context, String packageName) {
        try {
            final PackageManager manager = context.getPackageManager();
            final Signature[] signatures;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                final PackageInfo info = manager.getPackageInfo(
                    packageName,
                    PackageManager.GET_SIGNING_CERTIFICATES
                );
                if (info.signingInfo == null) {
                    return Collections.emptySet();
                }
                signatures = info.signingInfo.getApkContentsSigners();
            } else {
                @SuppressWarnings("deprecation")
                final PackageInfo legacy = manager.getPackageInfo(packageName, PackageManager.GET_SIGNATURES);
                @SuppressWarnings("deprecation")
                final Signature[] legacySignatures = legacy.signatures;
                signatures = legacySignatures;
            }
            return fingerprints(signatures);
        } catch (Exception error) {
            android.util.Log.w(
                "REM.PluginCertificate",
                "Unable to read current package signing certificates for " + packageName,
                error
            );
            return Collections.emptySet();
        }
    }

    public static Set<String> packageCertificateHistory(Context context, String packageName) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            return Collections.emptySet();
        }
        try {
            final PackageInfo info = context.getPackageManager().getPackageInfo(
                packageName,
                PackageManager.GET_SIGNING_CERTIFICATES
            );
            if (info.signingInfo == null || info.signingInfo.hasMultipleSigners()) {
                return Collections.emptySet();
            }
            final Set<String> history = fingerprints(
                info.signingInfo.getSigningCertificateHistory()
            );
            history.removeAll(currentPackageFingerprints(context, packageName));
            return history;
        } catch (Exception error) {
            android.util.Log.w(
                "REM.PluginCertificate",
                "Unable to read package signing history for " + packageName,
                error
            );
            return Collections.emptySet();
        }
    }

    public static String normalizeFingerprint(String value) {
        return value == null
            ? ""
            : value.replaceAll("[^0-9A-Fa-f]", "").toLowerCase(Locale.US);
    }

    private static String sha256(byte[] bytes) throws Exception {
        final byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
        final StringBuilder output = new StringBuilder(digest.length * 2);
        for (byte value : digest) {
            output.append(String.format(Locale.US, "%02x", value & 0xff));
        }
        return output.toString();
    }

    private static Set<String> fingerprints(Signature[] signatures) throws Exception {
        final Set<String> fingerprints = new HashSet<>();
        for (Signature signature : signatures == null ? new Signature[0] : signatures) {
            fingerprints.add(sha256(signature.toByteArray()));
        }
        return fingerprints;
    }
}
