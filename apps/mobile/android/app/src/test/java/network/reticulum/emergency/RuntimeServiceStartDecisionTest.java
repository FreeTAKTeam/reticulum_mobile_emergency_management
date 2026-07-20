package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;

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
}
