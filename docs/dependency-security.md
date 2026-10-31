# Development dependency security

The root `package.json` temporarily overrides rimraf's glob dependency to
14.0.3. This resolves GHSA-3g3m-2fjr-6pw4 (glob) and uses the compatible
minimatch 11 line that resolves GHSA-g3hv-75wg-m249.

This is a deliberate **cross-major compatibility override**, not an update
within rimraf 6's declared `^13.0.3` range. Do not override minimatch 11 under
glob 13: their internal matcher representations differ. Remove the override
when a compatible rimraf / Capacitor release declares a patched dependency
and the dependency audit and compatibility tests pass without it.

Capacitor CLI is a workspace-root development tool so npm applies the root
override consistently. The mobile workspace's `npx cap` commands still
resolve the root-installed CLI. Its version and the Capacitor runtime packages
are unchanged by this fix. Keeping CLI only in the linked mobile workspace
hits npm's workspace override issue (npm/cli#9514).

Run `npm ci`, `npm ls @capacitor/cli rimraf glob minimatch`,
`npm audit --audit-level=moderate`, and `npm run test:unit`. The dependency
compatibility tests exercise sync/async literal and glob deletion, ignored
paths, missing matches, string-path results and pre-aborted traversal. They
are bounded fixtures, not comprehensive upstream security proofs. Also check
mobile/web builds and `npx cap sync android` from `apps/mobile` when changing
this toolchain. Do not blindly use `npm audit fix --force`: the audit may
propose a breaking Capacitor CLI downgrade.

Primary references:
- https://github.com/advisories/GHSA-3g3m-2fjr-6pw4
- https://github.com/advisories/GHSA-g3hv-75wg-m249
- https://github.com/isaacs/node-glob/blob/v14.0.3/changelog.md
- https://github.com/npm/cli/issues/9514
