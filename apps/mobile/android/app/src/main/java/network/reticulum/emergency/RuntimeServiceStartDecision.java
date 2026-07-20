package network.reticulum.emergency;

final class RuntimeServiceStartDecision {
    enum Command {
        STOP,
        RESTORE_AFTER_BOOT,
        RESTORE_AFTER_PROCESS_RECREATION,
        KEEP_RUNNING
    }

    private RuntimeServiceStartDecision() {
    }

    static Command decide(String action, boolean shouldBeRunning) {
        if (ReticulumNodeService.ACTION_STOP_SERVICE.equals(action)) {
            return Command.STOP;
        }
        if (ReticulumNodeService.ACTION_RESTORE_AFTER_BOOT.equals(action)) {
            return Command.RESTORE_AFTER_BOOT;
        }
        if (action == null && shouldBeRunning) {
            return Command.RESTORE_AFTER_PROCESS_RECREATION;
        }
        return Command.KEEP_RUNNING;
    }
}
