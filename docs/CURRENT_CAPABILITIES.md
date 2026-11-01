# MIMIC — Current Capabilities

Updated: 2026-11-01 (Phase 1 Acceptance Completed & Phase 2 Windows Credential Manager & Account Profiles Delivered)

This document describes the **actual verified behavior** of MIMIC v0.1. It clearly distinguishes between Unit Tests, Integration Tests, and Manual GUI Tests.

---

## 1. Verified Capabilities

### Desktop Application, Storage & Persistence
- Built with Tauri 2, React 19, TypeScript, and Rust on Windows 11.
- Works 100% offline with LM Studio completely closed (local-first, deterministic execution).
- Project metadata and workflows persist across restarts in local storage. Closing and reopening MIMIC preserves all workflow steps, window scopes, and account associations.
- Privacy-safe storage: visual snippets are capped (< 128KB), full-screen captures are discarded after selection, and plaintext secrets are **never** written to project files, local storage, or version control.

### Application Scope & Process Identity
- **Separation of Application Identity from Screen State:** Window titles often transition dynamically (e.g., `MIMIC Automation Demo - Login Screen` → `Server Selection` → `Game Ready`). MIMIC decouples the application process identity from transient screen titles.
- **Strict Native Process Identity:** Before any input or inspection, MIMIC validates the window handle via `IsWindow`, `IsWindowVisible`, self-automation safety, PID matching, and full executable path verification (`QueryFullProcessImageNameW`). If process identity cannot be confirmed, the engine **fails closed**.
- **Scope Restriction on Overlay:** The Select on Screen transparent overlay clips all mouse interactions strictly to the authorized target window rectangle and PID, blocking accidental clicks on other desktop apps.
- **Forbid Blind Window Clicks:** A click action lacking locator criteria no longer clicks the center of the window blindly; it **fails closed** with an explicit error.
- **Run Workflow without Pre-selected Window:** When the first step is `launch-app`, the user can click **Run Workflow** without having to pre-select a window. The engine launches the executable, waits for the newly created process's window by PID, and locks onto it.

### Phase 2: Windows Credential Manager Integration (`Win32_Security_Credentials`)
- **Native OS Vault Integration:** Rust backend uses native Win32 `CredWriteW`, `CredReadW`, `CredDeleteW`, and `CredEnumerateW` APIs under the dedicated `MIMIC:` namespace.
- **Zero Plaintext Secrets:** Passwords entered in MIMIC are written directly to the Windows Credential Manager protected by DPAPI. Project files store only opaque vault target references (e.g. `vault:MIMIC:my_app:account_1`).
- **Memory Zeroization:** When executing `type-secret`, the secret string is read from the OS vault into memory at the exact moment of typing and explicitly overwritten with zeroes (`zeroize()`) immediately afterwards.
- **Active Type Secret Action:** Steps configured as `type-secret` focus the target child edit control, type the secret via discrete `WM_CHAR` messages, and avoid clipboard or plaintext leakage.

### Phase 2: User-Configurable Account Profiles
- **Dynamic Account Count:** Users can configure any number of account profiles (1, 2, 6, 10+) without hardcoding.
- **Dedicated Account Profiles Screen:** Manage profile name, username, password (saved directly to vault), target reference, notes, and active status.
- **One-Click Vault Verification:** Users can test credential retrieval from the OS vault directly in the UI with instant feedback.
- **Account Variable Interpolation:** Workflows automatically substitute `{username}` into `type-text` steps and reference the active account's vault target in `type-secret` steps.
- **Workspace Account Switcher:** Users can select the active account profile directly from the Workspace toolbar.

### Manual Teach / Select on Screen
- **Interactive Screen Overlay:** Native transparent overlay highlights target window controls with blue outlines and floating HUD badges.
- **Dual Selector Modes:** Supports Windows UI Automation (`IUIAutomation`) control detection as well as user-drawn rubberband regions for image template matching.
- **Accidental Click Protection:** Overlay intercepts and consumes mouse events, preventing unintended clicks to the underlying target.
- **Cancel Safety:** Pressing `ESC` or right-clicking immediately terminates selection mode without modifying the project.

### Emergency Stop
- **Responsive at All Times:** Emergency Stop can be triggered via UI button or global shortcut (`Escape` or `Pause` key).
- **Active Wait Loop Cancellation:** Halts wait loops (`wait-visible`, `wait-hidden`) within ≤100ms, immediately returning a safe `stopped` status without waiting for timeout expiration.

---

## 2. Test Verification Matrix

All tests have been executed and verified in the real Windows 11 environment:

| Test Layer | Scenario / Name | Verification Method | Status |
| :--- | :--- | :--- | :--- |
| **Unit Test** | `test_image_template_matching_algorithm` | Mathematical SAD & threshold confidence | **PASSED** |
| **Unit Test** | `test_real_screen_template_matching` | Real screen BMP crop matching with coordinate offset | **PASSED** |
| **Unit Test** | `test_prevent_duplicate_clicks_and_reliable_input` | Atomic click count == 1 & child control char typing | **PASSED** |
| **Unit Test** | `test_click_without_locator_fails_closed` | Empty locator click rejected safely | **PASSED** |
| **Unit Test** | `test_process_path_mismatch_fails_closed` | Spoofed or mismatched EXE paths rejected | **PASSED** |
| **Unit Test** | `test_emergency_stop_during_wait_visible` | Wait loop aborted immediately via atomic flag | **PASSED** |
| **Unit Test** | `test_launch_app_and_wait_window` | Spawns process and acquires window by PID | **PASSED** |
| **Unit Test** | `test_windows_credential_manager_roundtrip` | Write, read, enumerate, delete via Win32 Credential API | **PASSED** |
| **Unit Test** | `test_phase_2_type_secret_with_windows_credential_manager` | Dynamic vault resolution, typing, zeroization | **PASSED** |
| **Unit Test** | `test_phase_1_hardening_scenarios` | Permission check, locator resolution on resize | **PASSED** |
| **Integration Test** | `test_end_to_end_acceptance_workflow_7_steps` | 7-step sequence (Launch → Wait Login → Type User → Click Continue → Wait Server → Click Select → Verify Ready) on real Win32 demo window | **PASSED** |
| **Integration Test** | Target Process Identity & Safety | Rejection of mismatched PIDs, ungranted permission, closed window handles, and MIMIC's own window | **PASSED** |
| **Integration Test** | Windows Credential Manager OS Roundtrip | Credential stored in Windows Vault and retrieved by automation engine without plaintext exposure | **PASSED** |
| **Manual GUI Test** | Run without Pre-selected Window | Workflow starting with `launch-app` executes and locks onto target window without pre-selection | **PASSED** |
| **Manual GUI Test** | Persistence Across App Restart | Workflow, target config, and account profiles survive browser/app restart | **PASSED** |
| **Manual GUI Test** | Select on Screen Overlay Boundary | Overlay restricts highlight and selection strictly within authorized target window | **PASSED** |
| **Manual GUI Test** | Account Profiles Management | Create profile, save password to Credential Manager, switch active account, test retrieval | **PASSED** |
| **Manual GUI Test** | Offline Operation with LM Studio OFF | Entire Observe → Act → Verify lifecycle runs deterministically with LM Studio disconnected | **PASSED** |
| **Manual GUI Test** | Emergency Stop via Keyboard (`Escape`/`Pause`) | Halts active execution and wait loops immediately | **PASSED** |

### Compilation & Toolchain Status
- **Rust Backend:** `cargo test --manifest-path src-tauri/Cargo.toml` → **11 passed; 0 failed** (0 warnings).
- **TypeScript:** `npm.cmd run check` (`tsc --noEmit`) → **0 errors**.
- **Vite Web Build:** `npm.cmd run build:web` → **0 errors** (bundle built in 2.14s).

---

## 3. Not Implemented / Planned for Future Phases

- Multi-account sequential queue orchestration (running workflow sequentially for all active profiles in one batch).
- Project Export / Import with interactive credential remapping wizard.
- AI screen interpretation / VLM assisted authoring (Phase 3).
- Live-game automation (Yulgang W or commercial targets require prior terms and anti-cheat feasibility reviews).
