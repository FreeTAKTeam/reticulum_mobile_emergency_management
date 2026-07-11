package network.reticulum.emergency.plugin.api;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public final class CallerCertificateVerifierTest {
    @Test
    public void normalizesCertificateFingerprint() {
        assertEquals(
            "aabbccdd",
            CallerCertificateVerifier.normalizeFingerprint("AA:BB cc-DD")
        );
    }
}
