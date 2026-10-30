# Phase 0 — Technical Verification Checklist

**Status:** Not started. This checklist is for a Windows machine with LM Studio installed. No experiment below is marked as passed without evidence.

## Objective
Prove one small end-to-end loop on a **controlled test application**:
**Observe → choose an allowed action → act → verify → stop safely.**

Do not start with six game accounts or a production game. Screen capture and input automation may be limited in particular games or protected applications.

## Workstreams

### 0.1 Local AI connection
- [ ] Confirm LM Studio is installed and its local server can be started.
- [ ] Discover actual server URL, active model IDs, supported endpoints, and whether a **vision-capable** model is installed.
- [ ] Test Thai chat with timeout and clear offline error reporting.
- [ ] Test image understanding using a non-sensitive screenshot. A text-only LLM is not sufficient for image input.
- [ ] Record RAM/VRAM and latency while another permitted application is open.

**Pass:** Thai text request succeeds; image test succeeds only if the selected model truly supports images; failures are reported correctly.

### 0.2 Windows screen observation
- [ ] Enumerate available application windows and allow user selection.
- [ ] Capture only the selected test window with an obvious permission/recording indicator.
- [ ] Test resizing, moving, minimizing, DPI scaling, and app focus.
- [ ] Verify that screenshots stay on device and can be deleted.

**Pass:** Captures correspond to the intended test window; non-capturable windows fail safely.

### 0.3 Grounded input and verification
- [ ] Compare Windows UI Automation with coordinate-based input on a controlled demo app.
- [ ] Identify target element and its precondition before clicking.
- [ ] After the action, check for an observable postcondition; include timeout and retry limit.
- [ ] Implement a **non-LLM-dependent Emergency Stop** prototype.
- [ ] Ask for approval before executing actions; stop on any unexpected screen.

**Pass:** A simple demo task completes with verification, and Emergency Stop interrupts it immediately.

### 0.4 Teaching and persistence spike
- [ ] Record a small sequence of time-stamped mouse/keyboard events in the test app.
- [ ] Review and redact data before persisting.
- [ ] Produce a human-readable draft workflow; do not automatically mark it Ready.
- [ ] Store locally; reload and inspect it after restart.

**Pass:** Reopened workflow can be reviewed and its origin traced without retaining secrets.

### 0.5 Architecture decision
- [ ] Compare candidate desktop options (Tauri + web UI / .NET WPF) against validated Windows capture, automation, packaging, and maintainability.
- [ ] Choose the minimum viable backend/automation language based on measurements.
- [ ] Document the decision and tradeoffs; **do not assume the stack is already settled**.

## Phase 0 deliverables
1. A short technical spike report with tested Windows/LM Studio versions, model, results, and limitations.
2. One working Observe → Act → Verify demo on a controlled test application.
3. A documented architecture decision for Phase 1.
4. A prioritized Phase 1 implementation backlog.

## Security / privacy checkpoint
No real passwords, game-account credentials, sensitive screenshots, LM Studio model binaries, or recorded user input may be committed to this public repository. Ask the user before any external service or potentially destructive computer action.

## Next
After Phase 0 is verified, implement the first desktop UI, project management, local chat, and only then the recorded teach/review/replay loop described in `MIMIC_PROJECT_OVERVIEW.md`.
