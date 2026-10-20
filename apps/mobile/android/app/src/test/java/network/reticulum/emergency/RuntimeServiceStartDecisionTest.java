package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class RuntimeServiceStartDecisionTest {
    @Test
    public void explicitStartNeverSchedulesPersistedRestore() {
        assertEquals(
            RuntimeServiceStartDecision.Command.KEEP_RUNNING,
            RuntimeServiceStartDecision.decide(
                ReticulumNodeService.ACTION_START_RUNTIME,
                true
            )
        );
    }

    @Test
    public void nullStickyRestartRestoresDesiredRuntime() {
        assertEquals(
            RuntimeServiceStartDecision.Command.RESTORE_AFTER_PROCESS_RECREATION,
            RuntimeServiceStartDecision.decide(null, true)
        );
    }

    @Test
    public void stoppedRuntimeIsNotRestoredAfterProcessRecreation() {
        assertEquals(
            RuntimeServiceStartDecision.Command.KEEP_RUNNING,
            RuntimeServiceStartDecision.decide(null, false)
        );
    }

    @Test
    public void bootAndStopActionsRemainExplicit() {
        assertEquals(
            RuntimeServiceStartDecision.Command.RESTORE_AFTER_BOOT,
            RuntimeServiceStartDecision.decide(
                ReticulumNodeService.ACTION_RESTORE_AFTER_BOOT,
                true
            )
        );
        assertEquals(
            RuntimeServiceStartDecision.Command.STOP,
            RuntimeServiceStartDecision.decide(
                ReticulumNodeService.ACTION_STOP_SERVICE,
                true
            )
        );
    }

    @Test
    public void bootRestoreIgnoresBootCountChange() {
        assertTrue(RuntimeServiceStartDecision.shouldRestore(true, true, true, 4, 5));
    }

    @Test
    public void sameBootRestoreRequiresMatchingBootCount() {
        assertTrue(RuntimeServiceStartDecision.shouldRestore(false, true, true, 5, 5));
        assertFalse(RuntimeServiceStartDecision.shouldRestore(false, true, true, 4, 5));
    }

    @Test
    public void restoreRequiresDesiredRuntimeAndPersistedConfig() {
        assertFalse(RuntimeServiceStartDecision.shouldRestore(true, false, true, 5, 5));
        assertFalse(RuntimeServiceStartDecision.shouldRestore(true, true, false, 5, 5));
    }
}
