# Re-scoped Architecture: Browser-Only v1 with Local Connector

## Summary
This plan removes over-specification and aligns with the stated goal: start simple, keep Rust/Bevy, avoid a complex web IDE, and use a local connector so players can keep coding in their own IDE.

v1 runs all simulation in the browser. The backend is a single self-hosted Rust executable with SQLite for auth, metadata, and sync only.

## Explicitly Requested Scope
- Web app deployment for the game.
- Players log in and manage their own AI script repository.
- Keep UX mostly in Rust/Bevy rather than building a large custom web UI.
- Provide a small local connector app for syncing local code/artifacts.
- Enable races between players over time.
- Support tournaments in the future.
- Keep server-side simulation as a long-term goal, not a v1 requirement.

## Removed from v1 (Previously Over-Specified)
- Server-side race workers.
- Authoritative ranked simulation pipeline.
- Queue/worker orchestration for race execution.
- Replay validation pipeline tied to server simulation.
- Early hardening for untrusted code execution on server.

## v1 System Architecture

### 1) Browser Game Client (`racing-web`)
- Bevy/WASM build of the existing game.
- Runs emulator and physics in the browser.
- Loads bot ELF artifacts from the backend.
- Lets players run local races and shared race setups.

### 2) Backend (`racehub`) as a Single Executable
- One Rust HTTP server process with SQLite.
- Handles auth/session, users, script metadata, artifact storage, and race record publishing.
- Serves static web assets and API endpoints.
- Does not execute simulations.

### 3) Local Connector (`race-connector`)
- Small Rust CLI/daemon running on the player machine.
- Watches a configured local bot project/folder.
- Builds local RISC-V ELF artifacts.
- Uploads script versions and artifacts to backend.
- Keeps player workflow in any local IDE/editor.

## Core Flows

### Script Flow
1. Player edits bot code locally.
2. Connector builds and uploads the artifact.
3. Web app displays updated repository entries.

### Race Flow (v1)
1. Player selects artifacts and starts a race in browser.
2. Browser runs simulation locally.
3. Browser can upload a result summary/replay record for sharing.
4. Backend stores published records only; no server-side validation.

### Player-vs-Player (v1)
1. Player A loads Player B artifacts from backend.
2. Player A runs the race locally in browser.
3. Optionally publishes the race record.

## Public API Surface (v1)

### Auth
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/logout`
- `GET /api/v1/me`

### Scripts and Artifacts
- `GET /api/v1/scripts`
- `POST /api/v1/scripts`
- `POST /api/v1/scripts/{id}/versions`
- `POST /api/v1/artifacts/upload`
- `GET /api/v1/artifacts/{id}`

### Published Race Records (Metadata Storage)
- `POST /api/v1/race-records`
- `GET /api/v1/race-records/{id}`
- `GET /api/v1/race-records?user_id=&track_id=`

## Important Code/Interface Changes (Design Only)
1. Split bot artifact source behind an abstraction:
   - `LocalCompile` for native desktop workflow.
   - `RemoteArtifact` for web/WASM workflow.
2. Introduce a shared protocol crate (`race-protocol`) for API DTOs used by client, backend, and connector.
3. Add target gating:
   - `cfg(target_arch = "wasm32")` for web artifact loading path.
   - Keep native compile pipeline for desktop usage.

## Test Cases and Scenarios (for Implementation Phase)
1. Browser build can load track and multiple remote bot artifacts.
2. Connector detects local changes, builds ELF, and uploads new versions.
3. Login, artifact listing, and artifact download work end-to-end.
4. Player A can run local browser race against Player B artifact.
5. Published race record upload and retrieval works.
6. Backend first-run initializes SQLite schema and serves app/API.

## Assumptions and Defaults
- v1 uses trust-based published results (non-authoritative).
- Single self-hosted binary + SQLite is the only backend target in v1.
- Connector is the primary ingestion path in v1 (no web IDE).
- Tournaments and authoritative server-side simulation are deferred to later phases.
- Multiplayer in v1 is asynchronous artifact-vs-artifact races executed client-side.
