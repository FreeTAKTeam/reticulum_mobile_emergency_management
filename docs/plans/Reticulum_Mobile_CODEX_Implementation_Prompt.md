# Reticulum Mobile (Rust) -- CODEX Implementation Prompt

**Generated:** 2026-02-26T16:45:53.065210 UTC

------------------------------------------------------------------------

# ROLE

You are implementing a production-grade mobile Reticulum node using:

-   Rust (Reticulum-rs embedded)
-   UniFFI (Rust → Kotlin/Swift bindings)
-   Capacitor (Vue + TypeScript frontend)
-   Android (primary) + iOS (supported)

The phone MUST run Reticulum locally.

Your task is to implement the Rust core API, generate bindings,
integrate native layers, and expose a stable Capacitor plugin API for
Vue.

------------------------------------------------------------------------

# REPOSITORY STRUCTURE (MANDATORY)

Create this layout:

repo/ apps/ mobile/ \# Vue + Capacitor app android/ ios/ src/ packages/
node-client/ \# TypeScript wrapper around Capacitor plugin crates/
reticulum_mobile/ \# Rust UniFFI wrapper crate tools/ codegen/ \# uniffi
generation scripts

Reticulum-rs should be included as a git dependency inside
`reticulum_mobile`.

------------------------------------------------------------------------

# PHASE 1 -- RUST CORE (reticulum_mobile)

## 1. Create crate

-   Type: cdylib
-   Tokio runtime internally managed
-   Wrap Reticulum-rs runtime as a library (NOT daemon process)

## 2. Core Object

Implement:

struct Node - internal runtime - command channel - event broadcast
channel - state tracking

Public API:

-   new() -\> Node
-   start(config: NodeConfig)
-   stop()
-   restart(config: NodeConfig)
-   get_status() -\> NodeStatus
-   send(destination, data, options) -\> SendReceipt
-   broadcast(data, options) -\> SendReceipt
-   set_log_level(level)
-   subscribe_events() -\> EventSubscription

## 3. Event System

NodeEvent union:

-   StatusChanged
-   PeerChanged
-   PacketReceived
-   PacketSent
-   Log
-   Error

EventSubscription:

-   next(timeout_ms) -\> NodeEvent?
-   close()

Ensure: - Thread safe - No panics across FFI boundary - Structured error
types

## 4. Error Types

NodeError enum:

-   InvalidConfig
-   IoError
-   NetworkError
-   ReticulumError
-   AlreadyRunning
-   NotRunning
-   Timeout
-   InternalError

Never expose backtraces over FFI.

------------------------------------------------------------------------

# PHASE 2 -- UNIFFI INTEGRATION

Use UniFFI IDL or proc macros to define public interface.

Generate bindings for: - Kotlin - Swift

Create script:

tools/codegen/gen-uniffi.sh

Script must: - build Rust targets - run uniffi-bindgen - copy generated
sources into: - android project - ios project

Supported targets:

Android: - aarch64-linux-android - armv7-linux-androideabi -
x86_64-linux-android

iOS: - aarch64-apple-ios - aarch64-apple-ios-sim - x86_64-apple-ios-sim

------------------------------------------------------------------------

# PHASE 3 -- ANDROID IMPLEMENTATION

## Requirements

-   Foreground Service (required)
-   Persistent notification
-   Node lifecycle tied to service

Implement:

NodeManager (singleton) - Owns UniFFI Node - Coroutine polling
EventSubscription - Emits events to JS

Capacitor Plugin: ReticulumNode

JS Methods:

-   startNode(config)
-   stopNode()
-   restartNode(config)
-   getStatus()
-   send()
-   broadcast()
-   setLogLevel()

JS Events:

-   statusChanged
-   peerChanged
-   packetReceived
-   packetSent
-   log
-   error

Binary payloads must be Base64 encoded in JS.

Android 14+ Foreground Service compliance required.

------------------------------------------------------------------------

# PHASE 4 -- iOS IMPLEMENTATION

## Requirements

-   Foreground-first reliability
-   Clean suspend/resume handling
-   No assumption of infinite background runtime

Implement:

NodeManager (Swift) - Owns UniFFI Node - Polls EventSubscription on
background thread - Emits events to Capacitor bridge

Handle: - App entering background → persist state - App entering
foreground → resume node

Optional: integrate BackgroundTasks for deferred work.

------------------------------------------------------------------------

# PHASE 5 -- TYPESCRIPT WRAPPER

Create package:

packages/node-client

Implement class:

class ReticulumNodeClient

Features: - Typed methods - Event emitter abstraction - Auto base64
encode/decode - start() - stop() - on(event, callback) - sendBytes() -
sendString() - dispose()

Provide browser mock implementation for dev mode.

------------------------------------------------------------------------

# PHASE 6 -- TESTING

## Rust

-   start/stop idempotency
-   event channel does not deadlock
-   send emits PacketSent

## Android

-   Service starts
-   Node status transitions correctly

## iOS

-   Node start/stop works on simulator

## JS

-   Mock plugin allows Vue dev without device

------------------------------------------------------------------------

# SECURITY REQUIREMENTS

-   Never log secrets
-   No unvalidated file paths
-   Explicit opt-in for packet capture
-   No unwrap() across FFI boundary

------------------------------------------------------------------------

# SUCCESS CRITERIA

Implementation is complete when:

1.  Android device can:
    -   Start node
    -   Send packet
    -   Receive packet
    -   Remain running in foreground service
2.  iOS device can:
    -   Start node
    -   Send/receive while foregrounded
    -   Resume cleanly after suspension
3.  Vue app:
    -   Displays status
    -   Streams logs
    -   Sends test message

------------------------------------------------------------------------

# IMPLEMENTATION ORDER (STRICT)

1.  Build Rust core without UniFFI
2.  Add UniFFI bindings
3.  Verify Android static call works
4.  Add Foreground Service
5.  Add iOS binding
6.  Add Capacitor bridge
7.  Add TS wrapper
8.  Add tests

Do not skip order.

------------------------------------------------------------------------

END OF PROMPT
