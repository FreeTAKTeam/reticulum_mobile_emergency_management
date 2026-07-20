package network.reticulum.emergency;

import java.util.concurrent.atomic.AtomicLong;
import java.util.function.IntSupplier;

/** Serializes runtime mutations and prevents a queued restore from overwriting a newer request. */
final class RuntimeOperationGate {
    private final Object operationLock = new Object();
    private final AtomicLong generation = new AtomicLong();

    long snapshot() {
        return generation.get();
    }

    int runExplicit(IntSupplier operation) {
        generation.incrementAndGet();
        synchronized (operationLock) {
            return operation.getAsInt();
        }
    }

    boolean runRestore(long expectedGeneration, Runnable operation) {
        synchronized (operationLock) {
            if (generation.get() != expectedGeneration) {
                return false;
            }
            operation.run();
            return true;
        }
    }
}
