package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.Test;

public final class RuntimeOperationGateTest {
    @Test
    public void restoreRunsWhenNoNewerExplicitOperationExists() {
        final RuntimeOperationGate gate = new RuntimeOperationGate();
        final long generation = gate.snapshot();
        final List<String> operations = new ArrayList<>();

        assertTrue(gate.runRestore(generation, () -> operations.add("restore")));
        assertEquals(List.of("restore"), operations);
    }

    @Test
    public void explicitOperationInvalidatesQueuedRestore() throws Exception {
        final RuntimeOperationGate gate = new RuntimeOperationGate();
        final long restoreGeneration = gate.snapshot();
        final CountDownLatch explicitStarted = new CountDownLatch(1);
        final CountDownLatch releaseExplicit = new CountDownLatch(1);
        final List<String> operations = new ArrayList<>();
        final AtomicBoolean restoreRan = new AtomicBoolean(true);

        final Thread explicitThread = new Thread(() -> gate.runExplicit(() -> {
            explicitStarted.countDown();
            await(releaseExplicit);
            operations.add("explicit");
            return 0;
        }));
        explicitThread.start();
        assertTrue(explicitStarted.await(2, TimeUnit.SECONDS));

        final Thread restoreThread = new Thread(() -> restoreRan.set(
            gate.runRestore(restoreGeneration, () -> operations.add("restore"))
        ));
        restoreThread.start();
        releaseExplicit.countDown();

        explicitThread.join(2_000L);
        restoreThread.join(2_000L);
        assertFalse(explicitThread.isAlive());
        assertFalse(restoreThread.isAlive());
        assertFalse(restoreRan.get());
        assertEquals(List.of("explicit"), operations);
    }

    @Test
    public void explicitOperationRunsAfterRestoreAlreadyInProgress() throws Exception {
        final RuntimeOperationGate gate = new RuntimeOperationGate();
        final long restoreGeneration = gate.snapshot();
        final CountDownLatch restoreStarted = new CountDownLatch(1);
        final CountDownLatch releaseRestore = new CountDownLatch(1);
        final List<String> operations = new ArrayList<>();
        final AtomicBoolean restoreRan = new AtomicBoolean(false);

        final Thread restoreThread = new Thread(() -> restoreRan.set(
            gate.runRestore(restoreGeneration, () -> {
                restoreStarted.countDown();
                await(releaseRestore);
                operations.add("restore");
            })
        ));
        restoreThread.start();
        assertTrue(restoreStarted.await(2, TimeUnit.SECONDS));

        final Thread explicitThread = new Thread(() -> gate.runExplicit(() -> {
            operations.add("explicit");
            return 0;
        }));
        explicitThread.start();
        releaseRestore.countDown();

        restoreThread.join(2_000L);
        explicitThread.join(2_000L);
        assertFalse(restoreThread.isAlive());
        assertFalse(explicitThread.isAlive());
        assertTrue(restoreRan.get());
        assertEquals(List.of("restore", "explicit"), operations);
    }

    private static void await(CountDownLatch latch) {
        try {
            if (!latch.await(2, TimeUnit.SECONDS)) {
                throw new AssertionError("timed out waiting for test latch");
            }
        } catch (InterruptedException ex) {
            Thread.currentThread().interrupt();
            throw new AssertionError("test thread interrupted", ex);
        }
    }
}
