package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class JsonPayloadsTest {
    @Test
    public void fallsBackForMissingOrBlankPayloads() {
        assertEquals("{}", JsonPayloads.orFallback(null, "{}"));
        assertEquals("{}", JsonPayloads.orFallback("   ", "{}"));
    }

    @Test
    public void preservesTheOriginalNonBlankPayload() {
        assertEquals(" {\"running\":true} ", JsonPayloads.orFallback(" {\"running\":true} ", "{}"));
    }
}
