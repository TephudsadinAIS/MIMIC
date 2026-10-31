# MIMIC v0.1 — Workflow Data Contract (Draft)

> A **suggested** representation for the automation MVP. The next Coding Agent may refine the implementation after Windows experiments, but must document migrations and preserve per-step verification and secret separation.

## First principles
- **Projects** contain application scope and workflows; **workflow steps** describe generic UI actions.
- **Account profiles** are optional inputs, not six duplicated hardcoded workflows.
- **Steps** are serial and have observable pre/postconditions. Steps without verification must be explicitly marked as transitional and fail safely.
- A screenshot is a visual observation or matching template, **never** a safe place for passwords.
- Only user-approved target windows/processes are actionable.

## Suggested TypeScript model (not implemented)

\`\`\`ts
type Locator =
  | { kind: 'uia'; automationId?: string; name?: string; controlType?: string }
  | { kind: 'template'; assetId: string; region?: [number, number, number, number]; threshold: number }
  | { kind: 'relative-point'; x: number; y: number; referenceWidth: number; referenceHeight: number };

type Condition =
  | { kind: 'window-visible'; processId?: number; titleContains?: string }
  | { kind: 'element-visible'; locator: Locator }
  | { kind: 'element-absent'; locator: Locator };

type Action =
  | { kind: 'launch-app' }
  | { kind: 'wait-for'; condition: Condition }
  | { kind: 'click'; locator: Locator }
  | { kind: 'type-text'; text: string } // Never for passwords
  | { kind: 'type-secret'; secretRef: string } // OS vault lookup, no secret in workflow
  | { kind: 'press-key'; key: string };

type Step = {
  id: string;
  title: string;
  precondition?: Condition;
  action: Action;
  postcondition: Condition;
  timeoutMs: number;
  maxAttempts: number;
};

type Workflow = {
  schemaVersion: 1;
  id: string;
  projectId: string;
  name: string;
  steps: Step[];
};
\`\`\`

These are *illustrative types*, not a claim that selectors/visual locators work in a protected game. Strengthen the schema with runtime validation, per-action permission checks, bounded waits, a cancellation token, and window identity checks before implementing actual automation. Avoid using raw global process IDs as permanent identifiers; process IDs change.

## Minimal controlled demo

1. \`launch-app\` a sample demo window selected by user.
2. \`wait-for\` its "Login" view.
3. \`click\` a visible labeled control.
4. \`wait-for\` expected "Server selection" view.
5. \`click\` an allowed demo control.
6. Verify "Ready" view.

Use synthetic data; do not store a real account in the demo. The order is merely an example; allow steps to be added/reordered instead of hardcoding state names.

## Engine result states
\`idle → preflight → running → waiting → verified → completed\` with transitions to \`paused\`, \`needs-attention\`, \`failed\`, \`stopped\`. Include \`stepIndex\`, \`attempt\`, \`targetWindowId\`, timestamp, and sanitized human-readable explanation in run events.

## Storage rules
- Workflow/project metadata goes to application-local storage or SQLite (preferred before production); migration/versioning required.
- Images used for matching stay under app-local data paths and are explicitly approved.
- Passwords and recovery tokens live **only** in OS credential vault; workflow stores opaque references.
- Git tracks schemas and sanitized examples only, never individual projects, accounts, logs, or photos.

## Future compatibility with MIMIC AI
An AI teaching module can later **propose** draft workflow steps that use this same executor. The user reviews the draft and approves it before execution. The deterministic runner should not depend on the model.
