# REM Android Plugin SDK

REM Android plugins are independently installed APKs. They expose a bound service through the
versioned AIDL contract in this AAR; REM discovers that service, verifies the package signing
certificate, asks the operator to trust the publisher, and grants only explicitly approved REM
capabilities.

The current host API is `1.0`. Plugin processes do not receive raw Reticulum or LXMF objects.
They submit structured requests to the host and receive asynchronous responses identified by a
request ID.

See [`docs/plugins/android-plugin-system.md`](../../docs/plugins/android-plugin-system.md) for the
manifest, lifecycle, configuration, signing, and wire contracts.

Plugin services subclass `RemPluginService` and must configure both
`allowedHostPackageNames()` and `allowedHostCertificateFingerprints()`. Release builds should
source the REM host identity from local or CI signing configuration; a developer build may add
an explicit local debug fingerprint.
