package network.reticulum.emergency.plugins;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.Signature;
import android.content.pm.SigningInfo;
import java.security.MessageDigest;
import java.util.Set;
import network.reticulum.emergency.plugin.api.CallerCertificateVerifier;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.Shadows;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;

@RunWith(RobolectricTestRunner.class)
@Config(sdk = 28, manifest = Config.NONE, shadows = CallerCertificateAuthorizationTest.SigningShadow.class)
public final class CallerCertificateAuthorizationTest {
    private static final Signature OLD = new Signature(new byte[] {1});
    private static final Signature CURRENT = new Signature(new byte[] {2});

    @Test
    public void historicalHostCertificateCannotAuthorizeBinderCaller() throws Exception {
        final Context context = RuntimeEnvironment.getApplication();
        final PackageInfo info = new PackageInfo();
        info.packageName = "test.host";
        info.signingInfo = new SigningInfo();
        Shadows.shadowOf(context.getPackageManager()).installPackage(info);
        Shadows.shadowOf(context.getPackageManager()).setPackagesForUid(12345, new String[] {info.packageName});

        assertFalse(CallerCertificateVerifier.isAllowed(context, 12345, Set.of(digest(OLD))));
        assertTrue(CallerCertificateVerifier.isAllowed(context, 12345, Set.of(digest(CURRENT))));
        assertFalse(CallerCertificateVerifier.packageFingerprints(context, info.packageName).contains(digest(OLD)));
        assertTrue(CallerCertificateVerifier.packageCertificateHistory(context, info.packageName).contains(digest(OLD)));
    }

    private static String digest(Signature signature) throws Exception {
        final StringBuilder hex = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(signature.toByteArray())) {
            hex.append(String.format("%02x", value & 0xff));
        }
        return hex.toString();
    }

    @Implements(SigningInfo.class)
    public static class SigningShadow {
        @Implementation protected Signature[] getApkContentsSigners() { return new Signature[] {CURRENT}; }
        @Implementation protected Signature[] getSigningCertificateHistory() { return new Signature[] {OLD, CURRENT}; }
        @Implementation protected boolean hasMultipleSigners() { return false; }
    }
}
