# MIMIC — Provisional Desktop Architecture (v0.1)

**Decision status: temporary UI scaffold, not a locked technical choice.**

```text
src/App.tsx (React UI)
  ├── Projects / Workspace / Teach placeholder / Skills placeholder / Settings
  ├── src/services/projectStore.ts (only non-sensitive metadata, browser localStorage)
  └── src/services/localAi.ts (typed desktop adapter)
             │ tauri.invoke (only when running inside Tauri)
             ▼
src-tauri/src/lib.rs (Rust, narrow exposed commands)
  ├── ai_status: GET http://127.0.0.1:1234/v1/models
  └── ai_chat: POST http://127.0.0.1:1234/v1/chat/completions
             ▼
           LM Studio local server
```

## Why split the application this way?
- **UI must not know Windows input internals.** When observation/automation is implemented, add services and scoped commands rather than putting OS operations in React components.
- **One AI provider adapter.** This starter uses LM Studio via the native Rust layer. Future providers should implement an explicit interface, not add cloud fallbacks.
- **No silent access.** No screen capture, keyboard listeners, mouse injection, or microphone access is wired up.
- **Typed command contract.** The frontend invokes `ai_status` and `ai_chat` only; Rust enforces a fixed localhost endpoint and accepts only user/assistant roles.
- **Local-first.** No analytics or paid API dependencies have been added.

## What is implemented vs planned?

| Area | v0.1 | Next |
|---|---|---|
| Windows desktop shell | Tauri 2 source scaffold | Build/test on Windows |
| UI | Responsive React desktop layout | User feedback + usability testing |
| Projects | Save only names/descriptions in localStorage | SQLite and richer project settings |
| Local AI | Model listing + Thai text chat via Rust/LM Studio | Health checks, cancellation, model/VLM capabilities |
| Observation | Clearly disabled | Window picker; authorized capture |
| Teach | Clearly disabled | Event timeline and redacted demonstrations |
| Skills | Clearly disabled | Draft → Reviewed → Tested → Ready |
| Agent execution | Clearly disabled | Pre/postconditions, action gate, emergency stop |
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
- This UI shell is not yet a computer-use agent. All Teach/Play/Stop controls are disabled rather than pretending to work.
- A demonstration is not model weight training. Future skills should be versioned, human reviewed, and executed with explicit checks.
