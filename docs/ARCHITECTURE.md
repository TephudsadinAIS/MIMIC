# MIMIC — Provisional Desktop Architecture (v0.1)

**Decision status: temporary UI scaffold, not a locked technical choice.**

> **Current engineering priority (31 Oct 2026):** Develop the **non-AI Automation MVP** specified in [AUTOMATION_MVP_V01.md](AUTOMATION_MVP_V01.md). The existing LM Studio bridge remains optional; Windows automation must operate when LM Studio is closed.

### Native automation interfaces (controlled-demo proof)

```text
React Project / Workflow Builder / Run Monitor
  → Typed Desktop Commands
    → Permission + Scope Gate + Independent Emergency Stop
    → Workflow Runner (preconditions, actions, postconditions, bounded retry)
      → Windows Window/Process Adapter
      → UI Automation Adapter; Visual Matcher fallback when permitted
      → Credential Vault Adapter
      → Local Workflow/Run Storage
```

The executor is **deterministic**. An eventual AI agent may propose draft steps but must not be a dependency for executing reviewed workflows. Build a demo-app proof before trying any game.

```text
src/App.tsx (React UI)
  ├── Projects / local Workflow Builder / Run Monitor / Teach placeholder / Settings
  ├── src/services/projectStore.ts (non-sensitive project and workflow definitions in localStorage)
  ├── src/services/automation.ts (typed controlled-demo commands)
  └── src/services/localAi.ts (optional typed desktop adapter)
             │ tauri.invoke (only when running inside Tauri)
             ▼
src-tauri/src/lib.rs (Rust, narrow exposed commands)
  ├── ai_status: GET http://127.0.0.1:1234/v1/models
  ├── ai_chat: POST http://127.0.0.1:1234/v1/chat/completions
  └── automation_*: enumerate, create, observe, advance and stop the controlled Demo only
             ▼
           LM Studio local server
```

## Why split the application this way?
- **UI must not know Windows input internals.** When observation/automation is implemented, add services and scoped commands rather than putting OS operations in React components.
- **One AI provider adapter.** This starter uses LM Studio via the native Rust layer. Future providers should implement an explicit interface, not add cloud fallbacks.
- **No silent access.** No screen capture, keyboard listeners, mouse injection, or microphone access is wired up.
- **Typed command contract.** The frontend invokes narrow AI and controlled-demo automation commands only; Rust does not accept arbitrary coordinates, executable paths, or text input from the web UI.
- **Local-first.** No analytics or paid API dependencies have been added.

## What is implemented vs planned?

| Area | v0.1 | Next |
|---|---|---|
| Windows desktop shell | Tauri 2 source scaffold | Build/test on Windows |
| UI | Responsive React desktop layout | User feedback + usability testing |
| Projects | Save non-sensitive names, descriptions and controlled-demo workflow definitions in localStorage | SQLite and user-selected application settings |
| Local AI | Model listing + Thai text chat via Rust/LM Studio | Health checks, cancellation, model/VLM capabilities |
| Observation | Controlled Demo window identity and labeled state verification; no capture | UI Automation selector and authorized capture |
| Teach | Clearly disabled | Event timeline and redacted demonstrations |
| Skills | Clearly disabled | Draft → Reviewed → Tested → Ready |
| Agent execution | Controlled Demo workflow runner with bounded retries, local run log, pause/resume/stop and emergency stop | General target adapter and action executor |
| Voice | Clearly disabled | Local Thai STT/TTS |

## Phase 0 engineering constraints
1. Compare Tauri + Rust, native .NET/WPF, and a separate Windows automation sidecar based on real Windows tests.
2. Choose the most reliable Windows Automation/UIA and screenshot API for controlled test apps.
3. Add a user-visible **permission gate**, targeted window allowlist, and an **out-of-model emergency stop** before any action executor.
4. Prefer accessibility trees over visual coordinate guessing; add OCR/VLM only when justified.
5. Never persist raw passwords/credentials, captured input, private screenshots, or account lists in Git or unprotected localStorage.
6. Keep MIMIC general-purpose. Hearthstone-specific strategic analysis is a different application (MAVEN).

## Limitations
- Tauri compilation, Windows installers, and LM Studio interoperability **have not been verified in this session**; build and runtime tests must be recorded in `docs/PHASE_0_CHECKLIST.md`.
- The Workflow Builder currently runs only the controlled Demo Target. It is not a general Windows UI Automation engine, cannot launch arbitrary executables, and does not capture screens or accept credentials.
- A demonstration is not model weight training. Future skills should be versioned, human reviewed, and executed with explicit checks.
