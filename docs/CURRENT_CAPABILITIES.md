# MIMIC — Current Capabilities

Updated: 2026-10-31

This document describes the **actual current behavior** of MIMIC v0.1. It is not a product roadmap.

## What works now

### Desktop application and updates

- MIMIC is a Windows desktop application built with Tauri, React, and TypeScript.
- A Windows NSIS installer can be built locally. Installing a newer installer updates the existing MIMIC application; project data in the app's local storage is retained.
- Projects can be created and reopened. Each project stores only non-sensitive data locally: name, description, workflow definition, executable-path text, and target-window-title text.

### Local AI chat (optional)

- MIMIC can connect to an already-running LM Studio server at `http://127.0.0.1:1234`.
- It lists the models exposed by that server and sends text chat to the selected model.
- MIMIC can use the same loaded LM Studio model as another local application, such as MAVEN. A second model download or second model process is not required.
- `AI On / AI Off` applies only to MIMIC:
  - **AI On:** MIMIC may query the selected local model through LM Studio.
  - **AI Off:** MIMIC disables its chat controls and stops initiating model requests.
  - It does **not** stop LM Studio, unload the model, or affect MAVEN or another application using the same local server.
- Closing MIMIC closes its own UI and request client. It does not shut down LM Studio.

### Workflow editing and local storage

- A project includes an editable workflow list.
- Steps can be added, renamed, removed, and reordered.
- Each step exposes action, expected-state, timeout, and retry fields.
- Workflow changes save locally with the project.
- A project has an **Application Scope** section with:
  - executable path;
  - required window-title text.
- An opaque credential-reference type exists in the schema for future Windows Credential Manager integration. MIMIC does not store a password in project data, source files, or logs.

### Native safety harness (developer-only)

- The repository contains a controlled native Windows automation harness used for regression checks: `Login → Server selection → Ready`.
- It validates a known local test window, checks its expected control and next state, and fails closed on an unexpected state.
- This harness is **not exposed in the MIMIC product UI** and is not a workflow for a user application.

## What is visible but not executable yet

These parts are intentionally shown as configuration or disabled controls. They do not claim to work.

- Entering an executable path does not launch that application.
- Required window-title text is saved, but MIMIC does not yet locate or inspect that external window.
- `Run workflow`, target-window detection, and generic application launching are disabled until the native generic Windows adapter exists.
- Workflow step actions are not yet connected to Windows UI Automation, image/template matching, mouse input, keyboard input, or a Windows Credential Manager lookup.
- Run Monitor events currently apply only to the developer-native harness, not an external application.

## Not implemented

- File picker for selecting an EXE.
- General Windows application launcher and process/window allowlist.
- Windows UI Automation selectors for buttons, fields, or menus.
- Screen capture, OCR, template/image matching, or OpenCV.
- User teaching/recording mode.
- Secure account-profile editor and Windows Credential Manager integration.
- Typing credentials or any other secret into an application.
- General workflow execution, pause/resume/stop for external applications, and a system-wide emergency-stop hotkey.
- Sequential account queue / Run All.
- AI screen understanding, local VLM, voice, cloud AI, or background agent execution.

## Current use instructions

1. Install and open MIMIC.
2. Create or open a Project.
3. In **Application Scope**, enter the intended EXE path and expected window-title text. This is saved locally for the project.
4. Optionally open **Settings** and use the local AI chat if LM Studio is already running with a model.
5. Use `AI On / AI Off` to control only MIMIC's use of LM Studio.

At this stage, the project setup is a saved configuration. It does not automate the target application yet.

## Game-specific boundary

MIMIC does not currently launch, log in to, click through, or automate Yulgang W or any other game. Before considering a real-game workflow, its Terms of Service, anti-cheat behavior, and permitted automation scope must be checked. MIMIC will not bypass anti-cheat, CAPTCHA, authentication protections, or service restrictions.

## Privacy and data

- MIMIC is local-first. It does not send screen captures or project data to cloud services.
- Local AI chat is sent only to the local LM Studio endpoint when AI is enabled.
- Passwords must not be entered into project fields. Future secret use must rely on Windows Credential Manager references rather than plain text.
