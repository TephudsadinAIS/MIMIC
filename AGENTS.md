# MIMIC — Coding Agent Instructions

> Read this file before changing code. The repository is currently in planning / early development, not a shipped application.

## Source of truth
1. `MIMIC_PROJECT_OVERVIEW.md` — requirements, scope, safety, and phased plan.
2. `README.md` — product vision and user-facing overview.
3. `docs/PHASE_0_CHECKLIST.md` — first experiments and acceptance criteria.

If documents conflict or a product decision is missing, explain the conflict and ask rather than silently inventing requirements.

## Product rules
- MIMIC is a **Windows 11, local-first** assistant: observe a permitted app, teach by demonstration, review skills, execute with verification, and ask the user when uncertain.
- Use **LM Studio** first through a local AI provider adapter. Do not silently add a required paid/cloud API or upload screen captures.
- Persist projects, skills, event timelines, and run history locally. Stored demonstrations/workflows are *not* automatic LLM fine-tuning.
- Prefer explicit workflow rules and Windows accessibility information before OCR / computer vision / VLM. Never claim a screenshot alone guarantees correct action.
- The user controls Teach / Play / Pause / Stop / **Emergency Stop**. Emergency Stop must not depend on the LLM.
- Execute only within the allowed target application/window. Prompt before high-impact actions; fail closed when uncertain.
- Do not bypass game anti-cheat, CAPTCHA, authentication protections, or service restrictions. Do not implement game-specific automation until compliance and feasibility are checked.
- Hearthstone-specific strategy analysis is a **separate project**, not a requirement for the MIMIC core.

## Development workflow
1. Inspect the current repository before making changes.
2. Start with **Phase 0 verification**; do not scaffold a large application before checking Windows capture/input feasibility and the real LM Studio API.
3. Keep changes small, reversible, and testable. Document stack decisions based on verified constraints; desktop stack is **not finalized**.
4. Every significant change should include: what changed, how it was tested, results, and known limitations.
5. Never describe planned UI/features as already working.
6. Do not overwrite user work or commit machine-specific settings.

## Privacy and secrets — mandatory
- This repository is **public**. Never commit passwords, cookies, session tokens, account lists, recordings, private screenshots, personal data, local model files, or database contents.
- Keep all credentials out of prompts, logs, screenshots, code, and version control. Use a secure Windows credential store for secrets at runtime.
- Treat screen recording and keyboard event capture as sensitive; capture only after explicit user action and visibly show recording state.
- Prefer fake accounts and test windows in automated tests; redact traces before sharing.
- Follow `.gitignore`; do not assume it prevents secrets from being committed if force-added or already tracked.

## Definition of done
- Acceptance criteria defined and verified; automated checks run where available.
- Errors, timeouts, cancellation, and permissions handled.
- UI and README describe actual support accurately.
- No sensitive data or unapproved cloud dependencies are introduced.

## Current repository state (2026-10-30)
- `src/`: React UI and typed services. Home/Projects and text-chat UX are implemented in source; the Teach, Skill, Live Screen and Run surfaces are **disabled placeholders**.
- `src-tauri/`: Tauri 2 Rust shell with only `ai_status` and `ai_chat` commands reaching fixed localhost LM Studio. No arbitrary OS control exists.
- `docs/GETTING_STARTED.md`, `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_STATUS.md` explain how to build, what is not tested, and what remains.
- First agent priority: run `npm install`, `npm run check`, `npm run dev` on Windows; confirm local LM Studio; record issues before implementing capture/automation.
- Change the stack when verified technical constraints justify it. Preserve product boundaries, API isolation, privacy, and truthful UI states.
