# MIMIC — Current Capabilities

Updated: 2026-11-01 (Phase 1: Manual Teach & Select on Screen)

This document describes the **actual current behavior** of MIMIC v0.1. It is not a product roadmap.

## What works now

### Desktop application and updates

- MIMIC is a Windows desktop application built with Tauri, React, TypeScript, and Rust.
- Windows NSIS installer or binary can be built locally.
- Projects can be created, saved locally, and reopened.
- Project metadata is retained across restarts in local storage.

### Application Scope & Launching

- **Browse EXE:** A native Windows Open File Dialog (`IFileOpenDialog`) allows browsing and picking `.exe` files directly.
- **Launch Application:** Can launch the user-selected executable (or the built-in controlled demo window).
- **Target Window Enumeration:** Enumerate visible open Windows windows with process ID, title, and bounding dimensions.
- **Target Window Selection:** Dropdown selector maps the project's target application scope to an active window and automatically fills window title criteria.
- **Permission Gate:** Enforces user authorization before automation inputs are transmitted. Fails closed if the target window is missing, closed, or out of scope.

### Manual Teach / Select on Screen

- **Interactive Screen Overlay:** Clicking **Select on Screen** opens a native Windows transparent fullscreen overlay.
- **Visual Highlight:** Hovering over target window controls (buttons, inputs) highlights the element with a bright border and floating HUD description.
- **Accessibility & UI Automation:** Uses Windows UI Automation (`IUIAutomation`) to detect control types, names, automation IDs, and bounding rectangles.
- **Image Region Selection:** Users can click to select an element directly, or drag a rectangular rubberband box to crop an image region for visual matching.
- **Accidental Click Protection:** The overlay window intercepts all mouse clicks and keystrokes during selection mode, preventing accidental input to the target application.
- **Cancel Support:** Pressing `ESC` or right-clicking immediately exits selection mode with no side effects.
- **Visual Snippet Capture:** Selected elements or image regions automatically capture a base64 BMP image snippet for visual preview and verification in the Workflow Editor.

### Workflow Editor & Step Configuration

- **Step Configuration Modal:** Allows setting step title, action kind, target locator, parameters, expected result, timeout, and bounded retry policy (1–5 attempts).
- **Supported Action Kinds:**
  - `Click` — clicks element center or image region center
  - `Double Click` — double clicks element
  - `Type Text` — types non-sensitive text into focused or located edit control
  - `Type Secret` — uses opaque OS vault reference (`vaultKey`); passwords are never stored in plaintext
  - `Press Key` — sends virtual keys (`Enter`, `Tab`, `Esc`, `Space`, `Backspace`)
  - `Wait Until Visible` — polls until target element/state appears
  - `Wait Until Hidden` — polls until target element/state disappears
  - `Verify State` — verifies window title or element condition
  - `Launch Application` — opens target executable
- **Expected Result (Postcondition):** Allows setting expected state (e.g. Login, Server selection, Ready) or picking a verification element on screen.
- **Test Step:** Single-step execution button inside the modal and in the step list allows immediate verification of an action and its postcondition.
- **Step Management:** Reorder steps (Up/Down), edit, delete, and add steps manually or via Select on Screen.

### Shared Deterministic Automation Engine

- **Offline & No AI Required:** The automation runner executes 100% locally using native Win32 & UI Automation APIs. LM Studio can be completely closed.
- **Step Lifecycle:** Bounded retry, precondition check, action execution, and postcondition verification.
- **Execution Controls:** Run full workflow, Pause, Resume, Stop, and Emergency Stop.
- **Emergency Stop:** Immediate atomic stop halts execution and prevents any further Windows input.
- **Real-Time Monitor:** Color-coded event log with timestamps and status updates (`verified`, `completed`, `warning`, `error`).

### Controlled Demo Target

- Built-in multi-stage demo window (`MimicAutomationDemoTarget`) simulating Login (username edit, password edit, continue button) → Server selection → Ready.
- Can be used for end-to-end testing of Select on Screen, clicking, typing, and verification without touching commercial applications or games.

### Local AI Chat (Optional)

- MIMIC can optionally connect to an already-running LM Studio server at `http://127.0.0.1:1234`.
- AI features are completely decoupled from the automation engine.

## Not implemented yet / Next phases

- Windows Credential Manager OS vault backend integration (v0.1 uses opaque synthetic reference schema).
- Sequential multi-account profile queue (Phase 2).
- Project Export / Import with remapping wizard.
- AI screen interpretation / VLM assisted authoring (planned for Phase 3).
- Live-game automation (Yulgang W or commercial targets require prior terms and anti-cheat feasibility reviews).

## Security and Privacy Verification

- No cloud APIs, telemetry, or remote dependencies.
- No plaintext passwords in project JSON, source files, or logs.
- Overlay captures only upon user command and redacts sensitive field typing.
