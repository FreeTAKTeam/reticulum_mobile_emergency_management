package network.reticulum.emergency.plugin.api;

import android.app.Service;
import android.content.Intent;
import android.os.Binder;
import android.os.IBinder;
import android.os.RemoteException;
import androidx.annotation.Nullable;
import java.util.Set;

public abstract class RemPluginService extends Service {
    private final IRemPluginService.Stub binder = new IRemPluginService.Stub() {
        @Override
        public String getDescriptorJson() throws RemoteException {
            requireTrustedHost();
            return RemPluginService.this.getDescriptorJson();
        }

        @Override
        public void start(IRemPluginHost host, String sessionJson) throws RemoteException {
            requireTrustedHost();
            RemPluginService.this.onPluginStart(host, sessionJson);
        }

        @Override
        public void stop(String reason) throws RemoteException {
            requireTrustedHost();
            RemPluginService.this.onPluginStop(reason);
        }

        @Override
        public void onHostEvent(String eventJson) throws RemoteException {
            requireTrustedHost();
            RemPluginService.this.onHostEvent(eventJson);
        }

        @Override
        public void onHostResponse(String responseJson) throws RemoteException {
            requireTrustedHost();
            RemPluginService.this.onHostResponse(responseJson);
        }

        @Override
        public void handleConfigurationRequest(
            String requestJson,
            IRemPluginConfigurationCallback callback
        ) throws RemoteException {
            requireTrustedHost();
            RemPluginService.this.onConfigurationRequest(requestJson, callback);
        }
    };

    @Nullable
    @Override
    public final IBinder onBind(Intent intent) {
        return binder;
    }

    protected abstract Set<String> allowedHostCertificateFingerprints();
    protected abstract Set<String> allowedHostPackageNames();
    protected abstract String getDescriptorJson();
    protected abstract void onPluginStart(IRemPluginHost host, String sessionJson);
    protected abstract void onPluginStop(String reason);
    protected abstract void onHostEvent(String eventJson);
    protected abstract void onHostResponse(String responseJson);
    protected abstract void onConfigurationRequest(
        String requestJson,
        IRemPluginConfigurationCallback callback
    );

    private void requireTrustedHost() throws RemoteException {
        if (!CallerCertificateVerifier.isAllowed(
            this,
            Binder.getCallingUid(),
            allowedHostPackageNames(),
            allowedHostCertificateFingerprints()
        )) {
            throw new RemoteException("Untrusted REM host certificate");
        }
    }
}
