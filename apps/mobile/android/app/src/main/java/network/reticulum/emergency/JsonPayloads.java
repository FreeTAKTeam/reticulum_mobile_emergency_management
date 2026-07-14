package network.reticulum.emergency;

final class JsonPayloads {
    private JsonPayloads() {
    }

    static String orFallback(String raw, String fallback) {
        if (raw == null || raw.trim().isEmpty()) {
            return fallback;
        }
        return raw;
    }
}
