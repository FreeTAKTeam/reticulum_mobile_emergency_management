# Reticulum Mobile Emergency_Management

This app answers a simple question during an incident:

> How is everyone doing? What is happening?

It is designed to be simple enough for anyone to use, even under stress.

## Looks & Feel

<img width="438" height="881" alt="Dashboard" src="https://github.com/user-attachments/assets/1a5dfce2-8d41-469c-b9b5-ced8377e44ed" />
<img width="429" height="897" alt="Chat" src="https://github.com/user-attachments/assets/63b48784-5a2f-4fb1-b924-76e7d68fb571" />
<img width="451" height="878" alt="Positioning" src="https://github.com/user-attachments/assets/62e81b21-30a5-4d32-9051-26327f1083a7" />
<img width="433" height="898" alt="Status" src="https://github.com/user-attachments/assets/589ab34a-a1f5-4f90-bb2f-3ec5bb7a7520" />
<img width="442" height="899" alt="Events" src="https://github.com/user-attachments/assets/710f1061-e2a0-4d00-8d1e-56f3ecf44425" />


## What This App Does

- **Shares status updates** about people or teams (who is OK, who needs help, who is missing, etc.).
- **Works without any server**. Phones can form a peer-to-peer mesh of trusted peers and share updates directly.
- **Exchange encrypted Messages with Peers**. 
- **Sends logs of Events**. Short text messages with SITREP.
- **Stays compatible with RCH (Reticulum Community Hub)** if you want a directory to help discover peers, but it is not required. (in progress)
 
## Trust-Based Updates

This app assumes information is updated by the people who know the facts.

- Anyone in the mesh can create a status for someone and update it later.
- Example: if Joe created a status for Aunt Emma, Mary can update it after she visits her and has newer information.

The goal is one shared, evolving picture of the situation, not “who created the record”.

## Events / Logs

Alongside statuses, the app supports simple events and logs: short notes about conditions that affect the network or the response (for example, “power is out”, “bridge closed”, or “comms degraded”).

## Under The Hood

The network layer uses Reticulum, a secure mesh networking system. The core is implemented in Rust so it stays responsive on mobile devices.

## Install with Obtainium

Use Obtainium to track releases from this repository and install updates directly:

[![Add to Obtainium](https://img.shields.io/badge/Add%20to-Obtainium-3ddc84?style=for-the-badge&logo=android&logoColor=white)](https://apps.obtainium.imranr.dev/redirect.html?r=obtainium://add/https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management)

## Layout (For Developers)
- `apps/mobile`: Vue + Capacitor application shell.
- `packages/node-client`: TypeScript wrapper around the Capacitor plugin surface.
- `crates/reticulum_mobile`: Rust UniFFI wrapper crate.
- `tools/codegen`: Scripts for UniFFI code generation.

## End-to-End Testing

Playwright coverage runs the web build of `apps/mobile` and exercises the core operator flows in a browser.

1. Install browser binaries once: `npx playwright install chromium`
2. Run the suite from the repo root: `npm run test:e2e`
3. Use `npm run test:e2e:headed` when you want an interactive browser session
