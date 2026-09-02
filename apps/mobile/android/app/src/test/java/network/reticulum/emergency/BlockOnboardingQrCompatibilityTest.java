package network.reticulum.emergency;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import com.google.zxing.BinaryBitmap;
import com.google.zxing.MultiFormatReader;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.Result;
import com.google.zxing.ResultMetadataType;
import com.google.zxing.common.HybridBinarizer;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;

@RunWith(RobolectricTestRunner.class)
public final class BlockOnboardingQrCompatibilityTest {
    @Test
    public void levelMFixtureDecodesToMaximumRepresentablePayload() throws Exception {
        final byte[] expected;
        try (InputStream input = resource("/block-onboarding-max-v1.txt")) {
            expected = input.readAllBytes();
        }
        assertEquals(1_999, expected.length);

        final Bitmap image;
        try (InputStream input = resource("/block-onboarding-max-v1-level-m.png")) {
            image = BitmapFactory.decodeStream(input);
        }
        assertNotNull(image);
        final int width = image.getWidth();
        final int height = image.getHeight();
        final int[] pixels = new int[width * height];
        image.getPixels(pixels, 0, width, 0, 0, width, height);
        final BinaryBitmap bitmap = new BinaryBitmap(
            new HybridBinarizer(new RGBLuminanceSource(width, height, pixels))
        );
        final Result decoded = new MultiFormatReader().decode(bitmap);

        assertEquals(
            "M",
            decoded.getResultMetadata().get(ResultMetadataType.ERROR_CORRECTION_LEVEL)
        );
        assertArrayEquals(expected, decoded.getText().getBytes(StandardCharsets.UTF_8));
    }

    private static InputStream resource(String name) {
        final InputStream input = BlockOnboardingQrCompatibilityTest.class.getResourceAsStream(name);
        assertNotNull(input);
        return input;
    }
}
