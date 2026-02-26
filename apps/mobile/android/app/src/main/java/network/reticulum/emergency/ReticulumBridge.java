package network.reticulum.emergency;

public final class ReticulumBridge {
    static {
        System.loadLibrary("reticulum_mobile");
    }

    private ReticulumBridge() {}

    public static native int start(String configJson);
    public static native int stop();
    public static native int restart(String configJson);
    public static native String getStatusJson();
    public static native int connectPeer(String destinationHex);
    public static native int disconnectPeer(String destinationHex);
    public static native int sendJson(String payloadJson);
    public static native int broadcastBase64(String bytesBase64);
    public static native int setAnnounceCapabilities(String capabilityString);
    public static native int setLogLevel(String levelString);
    public static native int refreshHubDirectory();
    public static native String nextEventJson(int timeoutMs);
    public static native String takeLastErrorJson();
}
