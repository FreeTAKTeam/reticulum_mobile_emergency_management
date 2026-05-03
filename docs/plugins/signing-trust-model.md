# Signing And Trust Model

Native plug-ins are trusted code after loading, so signing controls install
trust. It is not a runtime sandbox.

## Modes

- Developer mode: unsigned packages may be installed.
- Production mode: unsigned packages are rejected unless the host is configured
  with an explicit unsigned override.

## Signature File

Signed packages include `signature.json` at the package root:

```json
{
  "plugin_id": "rem.plugin.example_status",
  "version": "0.1.0",
  "manifest_sha256": "...",
  "package_sha256": "...",
  "publisher": "FreeTAKTeam",
  "signature": "..."
}
```

The signature uses Ed25519. The host trusts publisher public keys configured in
REM, verifies the manifest hash, and verifies a canonical sorted package hash
that excludes `signature.json`.

## Packager

```powershell
cargo run --manifest-path tools/rem-plugin-packager/Cargo.toml -- `
  plugins/example-status-plugin `
  output/example-status.remplugin `
  --publisher FreeTAKTeam `
  --signing-key-base64 <32-byte-seed-base64>
```

The packager writes `signature.json` into the archive and does not modify the
source plug-in directory.
