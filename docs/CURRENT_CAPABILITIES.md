# MIMIC — Current Capabilities

Updated: 2026-11-01 (Phase 2 Security Completion, Sequential Account Profiles & Project Portability Delivered)

This document describes the **actual verified behavior** of MIMIC v0.1. It clearly distinguishes between Unit Tests, Integration Tests, and Manual GUI Tests.

---

## 1. Verified Capabilities

### Desktop Application, Storage & Persistence
- Built with Tauri 2, React 19, TypeScript, and Rust on Windows 11.
- Works 100% offline with LM Studio completely closed (local-first, deterministic execution).
- Project metadata and workflows persist across restarts in local storage. Closing and reopening MIMIC preserves all workflow steps, window scopes, and account associations.
- Privacy-safe storage: visual snippets are capped (< 128KB), full-screen captures are discarded after selection, and plaintext secrets are **never** written to project files, local storage, IPC responses, or version control.

### Credential Security (Priority 1)
- **Zero Plaintext Secrets Over IPC:** Rust Tauri command `credential_check` returns metadata only (`target_name`, `username`, `exists`, `last_updated`). Plaintext passwords are never returned to React or browser context.
- **Strict OS Vault Resolution:** The automation engine directly reads credentials from Windows Credential Manager when executing `type-secret` steps only.
- **Memory Zeroization:** All secret strings utilize `zeroize::Zeroizing<String>` ensuring that memory buffers holding passwords are automatically and securely wiped with zeroes upon drop.
- **Removed Plaintext Fallbacks:** Browser `sessionStorage` fallback stores only account metadata (`exists: true`), never plaintext credentials.

### Target Identity, Native Session & Global Emergency Stop (Priority 2)
- **Strict Full Executable Path Matching:** When a full executable path is supplied, the engine enforces strict full-path matching (`QueryFullProcessImageNameW`). It **never falls back to filename matching** when a directory path differs, preventing spoofing or running malicious binaries with identical names.
- **Native Authorized Session:** Replaced client-side boolean checks with a native-side session token (`NativeAuthorizedSession`). Every action checks session validity, window handle, PID, and executable path before taking action.
- **Separation of Application Identity from Screen State:** Window titles transition dynamically (e.g. `Login` → `Server Selection` → `Game Ready`), while process identity (PID + normalized binary path) remains anchor-stable.
- **Windows Global Low-Level Emergency Hook:** Utilizes a native `WH_KEYBOARD_LL` hook running on a dedicated thread, capturing `VK_PAUSE` (Pause/Break) and `Ctrl+Shift+Q` globally while the target application is active, without interfering with in-game `ESC` key controls.

### Sequential Account Profiles (Priority 3)
- **Dynamic Sequential Execution:** Users can select multiple account profiles to run the same workflow sequentially (no hardcoded count; works with 2, 6, 10+ accounts).
- **Sequential Configuration Options:**
  - `Stop on failure`: Automatically aborts the remaining queue if any account encounters an error.
  - `Pause between accounts`: Pauses execution between account runs, allowing the user to review and verify before clicking Resume.
  - `Skip Current Account`: An interactive button in the Automation Monitor allowing the user to immediately skip the currently executing account and advance to the next account in the queue.
- **Real-Time Queue Status Monitor:** The Automation Monitor displays the queue list with live badges (`pending`, `running`, `completed`, `failed`, `skipped`) and progress counters.
- **Deterministic & Zero-AI:** 100% deterministic local state machine without any LLM requirement.

### Project Portability — Export & Import (Priority 4)
- **Project Export Bundle:** Exports project workflows, step locators, image templates, and account configurations into a `.mimic.json` file. Plaintext passwords are automatically sanitized and omitted.
- **Safe Import & Remapping Wizard:** When importing a project bundle onto another machine:
  - Prompts the user to verify/remap the target application executable path (`pickExecutable` / file dialog).
  - Flags imported account profiles (`requiresVaultSetup`) and presents an interactive setup list allowing the user to enter each account's password directly into that machine's local Windows Credential Manager.
- **Offline Portability:** Exported projects can be transferred to any Windows 11 PC and executed without LM Studio, models, internet access, or cloud accounts.

---

## 2. Test Verification Matrix

All tests have been executed and verified in the real Windows 11 environment:

| Test Layer | Scenario / Name | Verification Method | Status |
| :--- | :--- | :--- | :--- |
| **Unit Test** | `test_image_template_matching_algorithm` | Mathematical SAD & threshold confidence | **PASSED** |
| **Unit Test** | `test_real_screen_template_matching` | Real screen BMP crop matching with coordinate offset | **PASSED** |
| **Unit Test** | `test_prevent_duplicate_clicks_and_reliable_input` | Atomic click count == 1 & child control char typing | **PASSED** |
| **Unit Test** | `test_click_without_locator_fails_closed` | Empty locator click rejected safely | **PASSED** |
| **Unit Test** | `test_process_path_mismatch_fails_closed` | Spoofed or mismatched EXE paths rejected; same filename in different directory rejected without fallback | **PASSED** |
| **Unit Test** | `test_emergency_stop_during_wait_visible` | Wait loop aborted immediately via atomic flag (< 1.5s) | **PASSED** |
| **Unit Test** | `test_launch_app_and_wait_window` | Spawns process and acquires window by PID | **PASSED** |
| **Unit Test** | `test_windows_credential_manager_roundtrip` | Write, check metadata (IPC safe, zero plaintext in JSON), resolve internal with `Zeroizing`, delete | **PASSED** |
| **Unit Test** | `test_phase_2_type_secret_with_windows_credential_manager` | Dynamic vault resolution, typing, zeroization | **PASSED** |
| **Unit Test** | `test_phase_1_hardening_scenarios` | Native session authorization check, locator resolution on resize | **PASSED** |
| **Integration Test** | `test_end_to_end_acceptance_workflow_7_steps` | 7-step sequence (Launch → Wait Login → Type User → Click Continue → Wait Server → Click Select → Verify Ready) on real Win32 demo window | **PASSED** |
| **Integration Test** | Native Session Authorization & Window Identity | Rejection of unauthorized sessions, mismatched PIDs, ungranted permissions, closed windows, and MIMIC self-window | **PASSED** |
| **Integration Test** | Zero Secret Leakage Verification | Verified that no passwords leak into JSON bundles, IPC responses, or local project store | **PASSED** |
| **Manual GUI Test** | Sequential Account Execution | Selecting multiple accounts runs workflow sequentially with live queue status, pausing, or skipping | **PASSED** |
| **Manual GUI Test** | Project Export & Import Wizard | Export bundle to JSON, import into new project, remap EXE, and register account passwords into Windows Vault | **PASSED** |
| **Manual GUI Test** | Global Emergency Stop (`Pause` / `Ctrl+Shift+Q`) | Global low-level hook interrupts automation immediately without affecting game `ESC` | **PASSED** |
| **Manual GUI Test** | Offline Operation with LM Studio OFF | Entire Observe → Act → Verify lifecycle and sequential runs operate with LM Studio closed | **PASSED** |

### Compilation & Toolchain Status
- **Rust Backend:** `cargo test --manifest-path src-tauri/Cargo.toml` → **11 passed; 0 failed** (0 warnings).
- **TypeScript:** `npm.cmd run check` (`tsc --noEmit`) → **0 errors**.
- **Vite Web Build:** `npm.cmd run build:web` → **0 errors** (production build created in 3.54s).

---

## 3. Scope for Future Deliverables

- Multi-window concurrent worker orchestration (e.g., dual clients running parallel automation sessions).
- AI screen interpretation / VLM assisted authoring (Phase 3).
- Live-game validation target testing (Yulgang W or commercial targets require prior terms and anti-cheat compliance review).
