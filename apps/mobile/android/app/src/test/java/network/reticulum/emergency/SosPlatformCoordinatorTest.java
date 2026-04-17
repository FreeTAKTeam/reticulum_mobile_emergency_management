package network.reticulum.emergency;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class SosPlatformCoordinatorTest {
    @Test
    public void recentLocationTimeAcceptsFreshFix() {
        final long now = 1_700_000_000_000L;

        assertTrue(SosPlatformCoordinator.isRecentLocationTime(now - 60_000L, now));
    }

    @Test
    public void recentLocationTimeRejectsStaleFix() {
        final long now = 1_700_000_000_000L;

        assertFalse(SosPlatformCoordinator.isRecentLocationTime(
            now - SosPlatformCoordinator.RECENT_LOCATION_MAX_AGE_MS - 1L,
            now
        ));
    }

    @Test
    public void recentLocationTimeRejectsMissingOrFutureFix() {
        final long now = 1_700_000_000_000L;

        assertFalse(SosPlatformCoordinator.isRecentLocationTime(0L, now));
        assertFalse(SosPlatformCoordinator.isRecentLocationTime(now + 1L, now));
    }

    @Test
    public void recentLocationSourcePrefersFreshGps() {
        final long now = 1_700_000_000_000L;

        assertEquals(
            SosPlatformCoordinator.LOCATION_SOURCE_GPS,
            SosPlatformCoordinator.selectRecentLocationSource(
                true,
                now - 120_000L,
                true,
                now - 10_000L,
                now
            )
        );
    }

    @Test
    public void recentLocationSourceFallsBackToFreshNetworkWhenGpsIsStale() {
        final long now = 1_700_000_000_000L;

        assertEquals(
            SosPlatformCoordinator.LOCATION_SOURCE_NETWORK,
            SosPlatformCoordinator.selectRecentLocationSource(
                true,
                now - SosPlatformCoordinator.RECENT_LOCATION_MAX_AGE_MS - 1L,
                true,
                now - 10_000L,
                now
            )
        );
    }

    @Test
    public void recentLocationSourceRejectsAllStaleLocations() {
        final long now = 1_700_000_000_000L;

        assertEquals(
            SosPlatformCoordinator.LOCATION_SOURCE_NONE,
            SosPlatformCoordinator.selectRecentLocationSource(
                true,
                now - SosPlatformCoordinator.RECENT_LOCATION_MAX_AGE_MS - 1L,
                true,
                now - SosPlatformCoordinator.RECENT_LOCATION_MAX_AGE_MS - 2L,
                now
            )
        );
    }

    @Test
    public void newerLocationTimeChoosesNewestPositiveTimestamp() {
        assertEquals(200L, SosPlatformCoordinator.newerLocationTime(100L, 200L));
        assertEquals(200L, SosPlatformCoordinator.newerLocationTime(200L, 0L));
        assertEquals(100L, SosPlatformCoordinator.newerLocationTime(0L, 100L));
    }
}
