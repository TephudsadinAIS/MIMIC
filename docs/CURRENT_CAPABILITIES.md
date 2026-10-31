# MIMIC — Current Capabilities

Updated: 2026-11-01 (Phase 1 Final Reliability & Hardening Verification)

This document describes the **actual current behavior** of MIMIC v0.1. It is not a product roadmap.

## What works now

### Desktop application and updates

- MIMIC is a Windows desktop application built with Tauri, React, TypeScript, and Rust.
- Windows NSIS installer or binary can be built locally.
- Projects can be created, saved locally, and reopened.
- Project metadata and workflows persist across restarts in local storage.
- Privacy-safe storage: Template match snippets are capped (< 128KB), full-screen captures are discarded, and plain text secrets are stripped before serialization.

### Application Scope & Launching

- **Browse EXE:** A native Windows Open File Dialog (`IFileOpenDialog`) allows browsing and picking `.exe` files directly.
- **Launch Application:** `launch_application` executes target executables or the built-in demo window.
- **Workflow Launch App:** Workflows can begin with a `launch-app` step without requiring a pre-opened window; the UI allows pressing **Run** immediately when the first step is `launch-app`, and the engine launches the EXE, matches the resulting window strictly by PID (`process_id == pid`), and automatically assigns the active target window for subsequent steps. Fallback to `project.target.executablePath` is provided if the step text is omitted.
- **Target Window Enumeration:** Enumerates visible open Windows windows with process ID, title, and bounding dimensions.
- **Target Window Selection:** Dropdown selector maps the project's target application scope to an active window and automatically fills window title criteria.
- **Strict Permission & Target Identity:** Requires explicit user consent before starting automation (`permission_granted` defaults to `false`). The engine performs strict identity verification (`IsWindow`, `IsWindowVisible`, self-automation safety, PID matching, executable path comparison, and window title validation) before every input action.

### Manual Teach / Select on Screen

- **Interactive Screen Overlay:** Clicking **Select on Screen** opens a native Windows transparent fullscreen overlay.
- **Visual Highlight:** Hovering over target window controls (buttons, inputs) highlights the element with a bright border and floating HUD description.
- **Accessibility & UI Automation:** Uses Windows UI Automation (`IUIAutomation`) to detect control types, names, automation IDs, and bounding rectangles.
- **Image Region Selection:** Users can click to select an element directly, or drag a rectangular rubberband box to crop an image region for visual template matching.
- **Accidental Click Protection:** The overlay window intercepts all mouse clicks and keystrokes during selection mode, preventing accidental input to the target application.
- **Cancel Support:** Pressing `ESC` or right-clicking immediately exits selection mode with no side effects.
- **Visual Snippet Capture:** Selected elements or image regions automatically capture a base64 BMP image snippet for visual preview in the Workflow Editor.

### Workflow Execution Engine & Action Kinds

- **Prevent Duplicate Clicks:** Every click action uses a unified single dispatch strategy. Single clicks on Win32 buttons use `BM_CLICK`, double clicks send discrete mouse messages to the child window, and coordinate clicks send client messages to the target window. Multiple overlapping click APIs are never dispatched together.
- **Reliable Text Input:** `type-text` requires an input locator, dynamically resolves the child edit control, sets focus via `WM_SETFOCUS`, and routes characters (`WM_CHAR`) directly to the target child control. Blind typing to the parent window is forbidden.
- **Real UIA Dynamic Locator Resolution:** During run, elements are dynamically resolved via `IUIAutomationTreeWalker` and Win32 child control enumeration (`EnumChildWindows`). Coordinates are calculated in real time, making execution resilient to window movement or repositioning.
- **Real Image Template Matching:** A native Rust template matcher using Sum of Absolute Differences (SAD) with localized Region of Interest (ROI) fast scan and full-window fallback. Tested against real screen captures with high confidence thresholds.
- **Real Postcondition Verification:** Actions must be verified by actual state or element changes before returning `verified`. Steps without explicit postconditions truthfully report `completed`, never claiming unverified success.
- **Wait Actions:**
  - `wait-visible` — actively polls until locator appears, with responsive timeout and window closure detection.
  - `wait-hidden` — actively polls until locator disappears.
  - `verify-state` — verifies immediate window title or element condition.
- **Supported Action Kinds:**
  - `Click` & `Double Click` — single-method dispatch with duplicate trigger prevention.
  - `Type Text` — focused child control character dispatch.
  - `Press Key` — virtual keys (`ENTER`, `TAB`, `ESC`, `SPACE`, `BACKSPACE`).
  - `Wait Until Visible` & `Wait Until Hidden`.
  - `Verify State`.
  - `Launch Application`.
- **Safe Secret Handling:** `type-secret` is safely disabled in both UI and Native Engine pending Phase 2 Windows Credential Manager integration. Mock passwords or plain text secrets are rejected with explicit security messages.
- **Emergency Stop & Retry Safety:**
  - Atomic emergency stop halts execution immediately during wait loops, before inputs, and during verification.
  - Retry safety guard: if an earlier attempt succeeded and postconditions are already met, the engine avoids repeating actions that could cause duplicate submissions.

### Test Verification Results

All tests have been executed and verified in the real Windows 11 environment:

#### 1. Unit Tests (`cargo test`)
- `test_image_template_matching_algorithm` — verifies pure mathematical SAD calculation and threshold confidence scoring: **PASSED**
- `test_real_screen_template_matching` — verifies real screen-captured pixels matching across window movement: **PASSED**
- `test_prevent_duplicate_clicks_and_reliable_input` — verifies atomic click count == 1 and character reception in target edit control: **PASSED**
- `test_phase_1_hardening_scenarios` — verifies permission denial, emergency stop, safe secret blocking, dynamic locator resolution after window move and resize: **PASSED**
- `test_launch_app_and_wait_window` — verifies process spawning and PID matching for window acquisition: **PASSED**

#### 2. Integration Tests
- **Win32 Demo Lifecycle:** Verified multi-stage Login -> Server selection -> Ready transition with synchronized state locks.
- **Target Process Identity:** Verified rejection of mismatched PIDs, ungranted permission, closed window handles, and MIMIC's own window.
- **Single-Dispatch Clicks:** Verified that button clicks fire exactly once and do not double-submit forms or buttons.

#### 3. Frontend & Build Verification
- TypeScript Check (`tsc --noEmit`): **0 errors**
- Web Production Build (`vite build`): **Success (283 kB JS, 21 kB CSS)**
- Rust Check (`cargo check`): **0 errors, 0 warnings**

### Controlled Demo Target & Automated Verification

- Built-in multi-stage demo window (`MimicAutomationDemoTarget`) simulating Login (username edit, password edit, continue button) → Server selection → Ready.
- End-to-end verified test cases:
  1. Window repositioned before run (dynamic resolution verifies at new coordinates).
  2. Window resized before action (button resolved and clicked successfully).
  3. Element not found (fails closed without false clicks).
  4. Expected result timeout (fails closed when expected state never appears).
  5. Target window closed during run (detected and aborted safely).
  6. Emergency stop during wait/run (aborts immediately with `stopped` status).
  7. 100% operational offline with LM Studio closed.

### Local AI Chat (Optional)

- MIMIC can optionally connect to an already-running LM Studio server at `http://127.0.0.1:1234`.
- AI features are completely decoupled from the deterministic automation engine.

## Not implemented yet / Next phases

- Windows Credential Manager OS vault backend integration (Phase 2).
- Sequential multi-account profile queue (Phase 2).
- Project Export / Import with remapping wizard.
- AI screen interpretation / VLM assisted authoring (planned for Phase 3).
- Live-game automation (Yulgang W or commercial targets require prior terms and anti-cheat feasibility reviews).

## Security and Privacy Verification

- No cloud APIs, telemetry, or remote dependencies.
- No plaintext passwords in project JSON, source files, or logs.
- Memory and storage sanitization prevents secret leaks and unbounded screenshot blobs.
