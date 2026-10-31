# Implementation Status

Updated: 2026-10-31.

## Committed source / configuration
- [x] React + TypeScript + Vite frontend scaffold
- [x] Tauri 2 native Rust shell configuration and narrow commands
- [x] Home, Projects, workspace, settings, Teach and Skill Library placeholders
- [x] Project metadata create/select in localStorage (no credentials)
- [x] LM Studio model list / Thai text-chat command implementation (source only)
- [x] Per-app Local AI toggle: disables MIMIC chat and local model requests without stopping the shared LM Studio server or other local applications.
- [x] Documentation for setup, architecture, and Phase 0

## Milestone A — Native automation harness (not exposed in product UI)
- [x] Native Windows window enumeration exposed through narrow Tauri commands.
- [x] Controlled native test target: `Login → Server selection → Ready`, using synthetic data only; retained only as a regression harness.
- [x] Explicit target selection and per-run permission checks are implemented in the native harness, not exposed as a product workflow.
- [x] Observe → Act → Verify command: validates the selected visible Demo window, its class, current state, and expected labeled control before issuing a window-scoped `BM_CLICK`; verifies the exact expected next state afterwards.
- [x] Fail-closed behavior for a missing window, unexpected state, or missing control; no global mouse input, coordinates, screenshots, or credentials are used.
- [x] Native Emergency Stop flag outside LM Studio and the automation action path.
- [x] Workflow Builder shell: local save/load, editable/reorderable steps, per-step timeout/retry fields, and a run-event monitor.
- [x] Native harness runner: Run, Pause, Resume and Emergency Stop state handling, bounded attempt counts, and exact state verification after every action.
- [x] Real application Project Target configuration: executable path and required window title; execution is visibly disabled until the generic native adapter exists.
- [x] Workflow schema reserves generic locator and secret-reference types without persisting password values.
- [ ] Generic target adapters, application picker/launcher, accessibility selectors, visual matching, and secure credential references (Milestone B/C; not implemented).
- [ ] Optional sequential profile queue only after single-profile demo passes.
- Current spec: [AUTOMATION_MVP_V01.md](AUTOMATION_MVP_V01.md); agent prompt: [PROMPT_GEMINI_AUTOMATION_V01.md](PROMPT_GEMINI_AUTOMATION_V01.md).

## Testing
- [x] `npm install` succeeds on Windows (2026-10-31; Node 24.14.0, npm 11.9.0).
- [x] `npm run check` and `npm run build:web` succeed on Windows (2026-10-31).
- [x] `cargo check` succeeds on Windows (2026-10-31).
- [x] Tauri desktop executable launches on Windows using `cargo run --no-default-features` (2026-10-31).
- [ ] LM Studio status and real Thai chat verified on Windows
- [x] Release NSIS installer builds on Windows: `src-tauri/target/release/bundle/nsis/MIMIC_0.1.0_x64-setup.exe` (2026-10-31; 2.78 MiB).
- [x] Automated native Observe → Act → Verify controlled demonstration: Login → Server → Ready, terminal fail-closed behavior, missing-target failure, and state observation (`cargo test`, 2026-10-31).
- [ ] UI end-to-end test of Workflow Builder and Run Monitor — pending explicit permission to control the test desktop.
- [ ] Configured-application target UI and schema verification — intentionally not run in this change at user request.

**No check box above is marked tested merely because its source was committed.**

## Future features — not implemented
Screen capture, recording, general Windows workflow execution, user-selected executable launching, UI Automation selectors, image matching, credential vault integration, and multi-account routines. The Emergency Stop currently applies only to the controlled Demo native command; it is not yet a general workflow runner hotkey.

## Next agent task
Build the current scaffold, fix verified errors, record evidence, then implement the scoped, no-AI demo Windows Automation experiment described in the new MVP specification; document verified limitations before real-game support.
