package network.reticulum.emergency;

import android.os.Bundle;
import android.util.Log;
import com.getcapacitor.BridgeActivity;

public class MainActivity extends BridgeActivity {
    private static final String TAG = "ReticulumNode";

    @Override
    public void onCreate(Bundle savedInstanceState) {
        // Register custom plugin before bridge startup.
        registerPlugin(ReticulumNodePlugin.class);
        super.onCreate(savedInstanceState);
        Log.i(TAG, "MainActivity initialized and ReticulumNode plugin registered.");
    }
}
