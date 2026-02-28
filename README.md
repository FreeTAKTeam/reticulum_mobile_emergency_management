# Reticulum Mobile Emergency_Management

This app answers a simple question during an incident:

> How is everyone doing? What is happening?

It is designed to be simple enough for anyone to use, even under stress.

## What This App Does

- **Shares status updates** about people or teams (who is OK, who needs help, who is missing, etc.).
- **Works without any server**. Phones can form a peer-to-peer mesh of trusted peers and share updates directly.
- **Stays compatible with RCH (Reticulum Community Hub)** if you want a directory to help discover peers, but it is not required.

## Trust-Based Updates

This app assumes information is updated by the people who know the facts.

- Anyone in the mesh can create a status for someone and update it later.
- Example: if Joe created a status for Aunt Emma, Mary can update it after she visits her and has newer information.

The goal is one shared, evolving picture of the situation, not “who created the record”.

## Events / Logs

Alongside statuses, the app supports simple events and logs: short notes about conditions that affect the network or the response (for example, “power is out”, “bridge closed”, or “comms degraded”).

## Under The Hood

The network layer uses Reticulum, a secure mesh networking system. The core is implemented in Rust so it stays responsive on mobile devices.

## Layout (For Developers)
- `apps/mobile`: Vue + Capacitor application shell.
- `packages/node-client`: TypeScript wrapper around the Capacitor plugin surface.
- `crates/reticulum_mobile`: Rust UniFFI wrapper crate.
- `tools/codegen`: Scripts for UniFFI code generation.
