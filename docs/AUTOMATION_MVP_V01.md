# MIMIC v0.1 — Automation First (No AI Required)

> **Current priority, 31 Oct 2026:** Build a working Windows **deterministic automation** MVP using the existing desktop application scaffold. Keep LM Studio integration optional and OFF by default. This document overrides the older AI-first ordering in `MIMIC_PROJECT_OVERVIEW.md` and `docs/PHASE_0_CHECKLIST.md` for the first delivery.
>
> **Important:** This is a build specification, NOT proof that the game is automatable or that the implementation is finished.

## 1. Goal

Let a Windows 11 user create a Project, select an application, configure an ordered workflow, and click **Run** to have MIMIC:

1. Launch an executable the user explicitly selected.
2. Wait for the corresponding window / screen state.
3. Perform an allowed UI action (click/select/type).
4. Verify that the expected next state appears.
5. Continue until the application reaches its desired screen, or **Pause / Ask / Stop** when unexpected conditions occur.

**Example use case: Yulgang / โยวกัง** — open game executable → log in → enter game → select server → reach in-game screen. These stages are *assumptions to validate using the actual permitted UI*. Do NOT embed game-specific screen coordinates or credentials into MIMIC core.

The user eventually wants multiple account profiles. The program should run the same workflow with different account aliases **sequentially**, only starting the next run after the previous account has reached a verified completion checkpoint. Existing game windows may remain open where the game permits it; do not assume that multiple simultaneous instances are supported.

**This is not an autonomous game-playing bot**, and daily quests, combat, bot evasion, CAPTCHA bypass, or anti-cheat circumvention are not part of v0.1. Check the relevant game's terms and technical protections before testing automation against the game.

## 2. Product design

### Project Home
- List projects and their last run status.
- New Project: name, optional description, user-selected executable path.
- A Project represents an allowed application + workflow(s), not a custom hardcoded game integration.

### Workflow Builder (new)
- Steps presented as a clear list: **Launch App → Wait for Login → Fill Credentials → Click Login → Wait for Server → Select Server → Wait for Game → Verify Ready**.
- Each step: **action**, **target**, **expected screen/window condition**, **timeout**, **retry policy**, and safe behavior when unknown.
- Configure through UI. No typing scripts necessary for the common operations. Advanced JSON export/editing is a future option.
- Step target kinds: Windows accessibility selector if available; screenshot template/region for canvas-like interfaces if necessary; absolute coordinates as a **last resort**, labeled brittle and bound to target-window dimensions.

### Accounts (optional after single-account flow succeeds)
- Account **aliases** and a reference to secrets in **Windows Credential Manager** (or another well-reviewed OS credential vault).
- Never store passwords in project JSON, localStorage, app logs, screenshots, LM Studio prompts, or GitHub.
- Account selection (e.g. 1 of 6 or all), **sequential run queue**, per-account status. No credentials in test fixtures.

### Run Monitor
- Current target window, active step, last verified condition, elapsed time, retries, and run log with sensitive data redacted.
- Controls: **Run, Pause, Resume, Stop, Emergency Stop**. Emergency Stop works even if the automation engine is blocked; it must not require AI.
- Show clear differences between **waiting**, **matched**, **executing**, **success**, **needs review**, and **failed**.
- No silent mouse/keyboard capture or cross-window clicks.

### Settings
- Application/window permission boundary (selected executable and target process/window).
- Manual calibration, screenshot/template preview, timeouts, local storage path.
- Local AI is shown as **Optional / Future**; app works with LM Studio completely closed.

## 3. Minimal UX journey (single-account)

1. Open MIMIC → Create Project → choose **Test App** or another authorized executable.
2. Add steps to workflow using UI. First test uses a controlled demo form (not a commercial game).
3. Save workflow locally.
4. Click Run → review scope and approve → open executable.
5. Detect target screen via UI Automation or template matching.
6. Execute one step → verify postcondition before advancing.
7. Show Completed or Needs Help. Never guess a target or click endlessly.
8. Reopen app → load saved workflow → run again without reconfiguring.

Once this works, test the same abstractions against a *permitted* game launcher. Only after single-account verification add a sequential account queue.

## 4. Technical boundaries

Preserve the current **Tauri 2 + React + TypeScript** shell for the first experiment unless a demonstrated Windows limitation requires change. Use a native automation service, not browser-only JavaScript, for Windows window enumeration/input. Keep the implementation behind explicit typed commands:

```text
React UI
  ├─ Project / Workflow Editor
  ├─ Run Monitor
  └─ Service adapter (typed invoke)
        ├─ Project & Workflow Storage (local, later SQLite)
        ├─ Workflow Runner (state machine)
        ├─ Windows Target Adapter (window/process enumeration + UIA)
        ├─ Visual Locator Adapter (template matching, opt-in)
        ├─ Secure Credential Adapter (Windows vault)
        └─ Stop/Permission/Logging layer
                ↳ controlled target application

Optional later: AI Provider Adapter → LM Studio (not in automation execution path)
```

Use an adapter for native Windows APIs. Prefer Windows UI Automation for desktop controls; for game-rendered canvases UIA may expose nothing, requiring scoped screenshot matching. Compare options with small tests instead of picking a computer-vision stack solely on preference. Never inject into or modify the game process, bypass protections, or intercept network traffic.

### Runner design

Each workflow step has:
- **Precondition:** required current screen/window.
- **Action:** launch / wait / click / type non-secret / retrieve-and-type secret / select / delay (bounded).
- **Postcondition:** observable state confirming success.
- **Timeout and bounded retry:** no unbounded loops or blind spam clicking.
- **Failure path:** pause and show a sanitized screenshot (only if the user approved capture), or stop with a reason.

Do not treat a fixed delay as proof of success. Do not activate input when the target window/process cannot be verified. Preserve cancellation across waits.

## 5. Milestones

### Milestone A — Working shell + native proof
- Build and launch the *existing* Tauri scaffold on Windows.
- Verify Project creation, UI navigation, optional text chat when LM Studio available.
- Create a safe local **demo target window** with clearly labeled buttons and state changes.
- Implement Windows window enumeration and target selection on this demo only.
- Add independent Emergency Stop behavior.
- **Pass:** after granting permission, MIMIC opens the demo, detects a known button, clicks, and verifies the new state. If screen is unexpected it stops.

### Milestone B — Reusable workflow engine
- Define typed workflow data, schema validation, and local persistence.
- Add UI for adding/reordering/editing steps and previewing pre/postconditions.
- Run, Pause, Resume, Stop, timeout/retry with observable status.
- Unit tests with mock window/input adapter (no actual process required).
- **Pass:** save/reload a 3–5 step workflow and complete it on the demo, including simulated failure and Stop.

### Milestone C — Single account and credential safety
- Use Windows Credential Manager rather than plain-text credentials.
- Test form filling on the controlled demo with synthetic credentials.
- Verify no password leaks in screenshots, logs, repo, or exception text.
- **Pass:** safe single-profile end-to-end from launch to expected final state.

### Milestone D — Allowed real target and sequential profiles
- Validate real target's terms, actual screens, window targeting, compatibility, and constraints.
- Support optional account aliases and sequential queue.
- **Pass:** one verified real-app workflow on explicitly authorized software; only then expand to multiple profiles.

## 6. Explicitly out of scope

- LLM/VLM inference, LM Studio startup, model training, Teach by Demonstration/recording, voice, autonomous screen interpretation, multi-app planning, daily quests, combat, and arbitrary script execution.
- Game bot evasions and unauthorized account automation.
- UI claiming a feature is live before it exists.

## 7. Definition of done

1. Builds and runs on Windows 11; tests and exact commands documented.
2. Demo app workflow performs an Observe → Act → Verify cycle with no AI.
3. Workflow saved locally and can be replayed after restart.
4. Fail-safe on unknown screen; cancellation works reliably.
5. Target process/window scope enforced; sensitive content is never written to source/Git.
6. All UI controls either work or clearly state **Not implemented**.
7. The coding agent reports any unsupported game-specific behavior without fabricating success.

## 8. Order of authority for v0.1

1. `AGENTS.md` — agent behavior and safety.
2. **This document** — immediate build scope and acceptance criteria.
3. `docs/WORKFLOW_CONTRACT_V01.md` — tentative workflow representation.
4. `docs/ARCHITECTURE.md` — repository structure and adapters.
5. `MIMIC_PROJECT_OVERVIEW.md` — long-term AI teaching vision.

If something conflicts with the v0.1 scope, follow this document and mention the discrepancy. Architectural details may change after evidence from Windows experiments.
