# reticulum_mobile_emergency_management

Monorepo scaffold for mobile emergency management clients backed by Reticulum.

## Layout
- `apps/mobile`: Vue + Capacitor application shell.
- `packages/node-client`: TypeScript wrapper around the Capacitor plugin surface.
- `crates/reticulum_mobile`: Rust UniFFI wrapper crate.
- `tools/codegen`: Scripts for UniFFI code generation.
