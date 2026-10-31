# MIMIC — Offline / No-AI Portability and Configurable Profiles

**Status:** Product requirements approved in conversation, 31 October 2026. Not implemented or verified on a second computer yet.

## Non-negotiable product requirement

**MIMIC's normal Windows Automation features must work without a model, LM Studio, internet access, AI API keys, NVIDIA GPU, or account subscription.** A user can install MIMIC on a different Windows computer that has no AI software and execute previously configured deterministic workflows.

AI is an optional extension that may suggest or explain workflows, but the validated workflow runner, screen/window detection, action execution, local persistence, and safety controls must not call the LLM or block while trying to discover one.

A missing or disabled AI runtime must result in **AI Unavailable**, never **Automation Unavailable**.

## Runtime separation

```text
MIMIC Windows App
 ├─ Required: Project/Workflow UI
 ├─ Required: Native Automation Runner
 │   ├─ User-approved app/window scope
 │   ├─ Window/element observation + optional local image templates
 │   ├─ Precondition → action → postcondition
 │   ├─ Bounded retry, timeout, pause, stop, emergency stop
 │   └─ Local project & run storage
 └─ Optional: AI Assistant
     └─ LM Studio / LLM / VLM
```

- Do not start LM Studio on launch or make a startup AI connection a prerequisite for using workflows.
- No need to download an AI model or install Python just to run MIMIC (unless the build is explicitly verified to require such a runtime; prefer bundling native dependencies).
- Any visual match algorithm used for basic automation must be **local and non-LLM**; provide its required assets as part of the installed application or an importable workflow package.
- Existing local-chat UI may remain and display Offline status.

## Account/profile design (optional, configurable)

**Multiple accounts are not compulsory.** A Project may have zero account profiles, one profile, or any practical number configured by its user. Never hardcode six accounts.

- Workflow supports a single execution with manual inputs, or a selected set of profiles.
- Account profiles are explicitly user-created and enable/disable individually.
- A queue, if enabled, runs chosen profiles **sequentially**; optionally stop on first failure or skip a failed profile with user approval.
- Per-profile inputs (e.g., username alias, desired server) are workflow variables.
- **Passwords are not workflow variables.** Store credentials only in the Windows Credential Manager (or equivalent OS vault) and save opaque references in workflow metadata.
- Do not presume that a game supports multiple windows or multiple accounts. Verify technical constraints, terms, and allowed automation before any live-game integration.

Example: Project "โยวกัง" with six profile aliases, all selected. Workflow "Enter game": launch EXE → wait Login → enter credentials → submit → wait Enter Game → choose Server → verify in-game Ready. Leave completed game windows open only if permitted and supported. The **same single workflow** is reused for selected profiles; user may choose 1, 2, 6, or none.

## Move to another computer

There are two distinct scenarios:

**A. Install MIMIC on another PC:** ship a Windows installer (e.g. NSIS) that includes all **MIMIC-owned** native code and assets. Confirm Windows prerequisites, notably the WebView2 runtime for Tauri, and provide friendly installation/error messages where missing. It must work with LM Studio never installed.

**B. Move workflows/projects between PCs:** design explicit **Export Project** / **Import Project** with versioned schema, local template images and asset references, but **exclude secrets, tokens, local log files, and private screenshots by default**. Validate paths and remap the selected EXE location and window constraints on the destination computer. Credentials must be re-entered into that computer's Windows Credential Manager; they cannot be carried over as plaintext or silently migrated. Export/import is a later feature, not currently present.

**Note:** "Windows installer works without AI" does **not** mean that any single EXE will be portable without installing dependencies. Treat a standalone portable build as a separate packaging target requiring real tests. The target application/game itself must also be installed and compatible on the second machine.

## Acceptance tests

1. Windows 11 test PC with **no LM Studio, no local model, no NVIDIA GPU, and no internet** can install/open MIMIC.
2. Create a project, configure and save an approved demo-app workflow, restart MIMIC, and successfully run it.
3. Normal workflow actions and the Emergency Stop work with AI set to OFF or AI never installed.
4. Enable optional AI UI while LM Studio is missing → only AI features show offline; all Automation UI remains functional.
5. Profiles: run a single workflow with zero/one/multiple selected **synthetic profiles** using a controlled demo app; queue order and per-profile states are visible.
6. If import/export is implemented: template assets and nonsecret settings import successfully; EXE paths must be mapped; secrets must be re-entered.
7. Confirm no cloud APIs, account subscription, model downloads, or hardcoded six-profile logic were added.

## Agent implementation rules

- For current work, prioritize the core runner and demo-app acceptance test over AI/voice.
- Retain Tauri + React scaffolding unless experiments justify switching stacks.
- Do not report portability or offline support as verified until explicitly tested on an environment without AI.
- Update `docs/IMPLEMENTATION_STATUS.md` with measured results.
