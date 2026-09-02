package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public final class BatteryPowerCoordinatorTest {
    @Test
    public void forwardsPercentAndChargingToNativeSink() {
        final FakeObservation observation = new FakeObservation();
        final FakeSink sink = new FakeSink();
        final BatteryPowerCoordinator coordinator = new BatteryPowerCoordinator(observation, sink);

        coordinator.start();
        observation.emit(3, 4, true);

        assertEquals(75, sink.percent);
        assertEquals(true, sink.charging);
        assertEquals(1, sink.updateCount);
    }

    @Test
    public void ignoresInvalidBatteryScale() {
        final FakeObservation observation = new FakeObservation();
        final FakeSink sink = new FakeSink();
        final BatteryPowerCoordinator coordinator = new BatteryPowerCoordinator(observation, sink);

        coordinator.start();
        observation.emit(10, 0, false);

        assertEquals(0, sink.updateCount);
    }

    @Test
    public void receiverLifecycleIsIdempotent() {
        final FakeObservation observation = new FakeObservation();
        final BatteryPowerCoordinator coordinator = new BatteryPowerCoordinator(
            observation,
            new FakeSink()
        );

        coordinator.start();
        coordinator.start();
        coordinator.close();
        coordinator.close();

        assertEquals(1, observation.startCount);
        assertEquals(1, observation.closeCount);
    }

    private static final class FakeObservation implements BatteryPowerCoordinator.BatteryObservation {
        private BatteryPowerCoordinator.BatteryListener listener;
        private int startCount;
        private int closeCount;

        @Override
        public void start(BatteryPowerCoordinator.BatteryListener listener) {
            this.listener = listener;
            startCount += 1;
        }

        void emit(int level, int scale, boolean charging) {
            listener.onBatteryChanged(level, scale, charging);
        }

        @Override
        public void close() {
            closeCount += 1;
            listener = null;
        }
    }

    private static final class FakeSink implements BatteryPowerCoordinator.NativePowerSink {
        private int percent;
        private boolean charging;
        private int updateCount;

        @Override
        public int updateBatteryState(int percent, boolean charging) {
            this.percent = percent;
            this.charging = charging;
            updateCount += 1;
            return 0;
        }
    }
}
