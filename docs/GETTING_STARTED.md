# MIMIC v0.1 — Getting Started (Windows 11)

> **Current state:** UI scaffold with local project metadata and a real LM Studio text-chat bridge. Screen observation, recording, speech, and computer control do **not** exist yet. The desktop app has not been built or run on the user's Windows computer by ChatGPT.

## Why this scaffold?
The product requirements are in `MIMIC_PROJECT_OVERVIEW.md`. This is a **provisional** Tauri 2 + React + TypeScript starting point, deliberately separated into UI, adapter, and narrow native commands. An implementation agent can replace the desktop shell if Phase 0 experiments demonstrate a better choice.

## Prerequisites
- Windows 11
- Node.js 22 LTS or compatible Node version for the pinned project dependencies
- Rust stable with MSVC toolchain (`rustup`); Visual Studio C++ Build Tools and Windows SDK for Tauri native builds
- Microsoft Edge WebView2 Runtime (normally present on Windows 11)
- LM Studio installed; start its **Local Server** and load a compatible text model when you want to test chat

## Run desktop app
Open **PowerShell** in the cloned repository directory:

```powershell
git clone https://github.com/TephudsadinAIS/MIMIC.git
cd MIMIC
npm install
npm run dev
```

The `npm run dev` command starts the **Tauri Windows desktop window** and its Vite development UI. It does not automate anything on screen.

## Optional browser-only UI preview
```powershell
npm run dev:web
```
Open `http://127.0.0.1:1420`.

**Important:** Browser preview intentionally does not invoke the native LM Studio bridge, so it displays “Browser Preview” instead of claiming to be connected. Project creation still works in the preview.

## Check LM Studio
1. In LM Studio, start the local server at `http://127.0.0.1:1234` and load a chat model.
2. In MIMIC Desktop, open **Settings → Check connection**.
3. Choose a model if multiple IDs are returned.
4. Create a Project, open it, and send a message in **MIMIC Chat**.
5. If the server is offline or no model is loaded, the UI will show a genuine error or disabled chat.

This version only uses **text chat**. A vision-capable model and Windows capture must be tested separately.

## Frontend checks and release build
```powershell
npm run check
npm run build:web
npm run build
```

`npm run build` invokes the Tauri native Windows build and NSIS packaging. If the packager requires Windows icon files, generate platform icons from a suitable square PNG (e.g. via `npm exec tauri icon path/to/square-icon.png`) and commit them in a separate asset commit. The starter currently has no pre-generated Windows icon.

## Current functionality
- Actual: desktop shell source, pages, project creation/selection, browser-local metadata, local server status, and local chat bridge through Rust.
- **Not implemented:** screen capture, mouse/keyboard control, Teach/Record, skill memory, run/play, safety engine, voice, game automation.
- Project names/descriptions persist only in localStorage. Chat messages stay in memory during the current app session and are **not** persisted.
- Do not enter passwords or sensitive material in project metadata or chat.

## Next instruction for an AI coding agent
Read `AGENTS.md`, `MIMIC_PROJECT_OVERVIEW.md`, `docs/PHASE_0_CHECKLIST.md`, and `docs/ARCHITECTURE.md`. Then run this scaffold locally, report build errors, confirm LM Studio behavior, and perform Phase 0 observation/input experiments using a controlled test application before enabling any automation.

Do not treat generated code as proven to run until the tests above are performed on the target Windows machine.
