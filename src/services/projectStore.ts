import type { AccountProfile, CredentialReference, Project, ProjectExportBundle, ProjectTarget, Workflow, WorkflowStep } from '../types';

const STORAGE_KEY = 'mimic.project-metadata.v1';

export function createApplicationTarget(): ProjectTarget {
  return { kind: 'configured-application', executablePath: '', windowTitleContains: '', targetWindowId: null };
}

export function createDemoWorkflow(id: string): Workflow {
  return {
    schemaVersion: 1,
    id: `${id}-workflow`,
    name: 'End-to-End Login & Server Workflow',
    steps: [
      {
        id: `${id}-step-1-launch`,
        title: 'Launch Application',
        kind: 'launch-app',
        actionType: 'launch-app',
        text: 'DEMO',
        expectedState: 'login',
        timeoutMs: 10_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-2-wait-login`,
        title: 'Wait for Login Window',
        kind: 'wait-visible',
        actionType: 'wait-visible',
        expectedState: 'login',
        locator: { kind: 'uia', automationId: '101', name: 'Continue to Server', controlType: 'Button' },
        timeoutMs: 10_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-3-type-user`,
        title: 'Type Username',
        kind: 'type-text',
        actionType: 'type-text',
        text: 'player_one',
        locator: { kind: 'uia', automationId: '102', controlType: 'Edit' },
        timeoutMs: 5_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-4-click-login`,
        title: 'Click Continue Button',
        kind: 'click',
        actionType: 'click',
        expectedState: 'server',
        locator: { kind: 'uia', automationId: '101', name: 'Continue to Server', controlType: 'Button' },
        timeoutMs: 5_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-5-wait-server`,
        title: 'Wait for Server Selection',
        kind: 'wait-visible',
        actionType: 'wait-visible',
        expectedState: 'server',
        locator: { kind: 'uia', automationId: '101', name: 'Select Test Server', controlType: 'Button' },
        timeoutMs: 10_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-6-click-server`,
        title: 'Click Select Server Button',
        kind: 'click',
        actionType: 'click',
        expectedState: 'ready',
        locator: { kind: 'uia', automationId: '101', name: 'Select Test Server', controlType: 'Button' },
        timeoutMs: 5_000,
        maxAttempts: 1,
      },
      {
        id: `${id}-step-7-verify-ready`,
        title: 'Verify Game Ready',
        kind: 'verify-state',
        actionType: 'verify-state',
        expectedState: 'ready',
        timeoutMs: 5_000,
        maxAttempts: 1,
      },
    ],
  };
}

function readWorkflow(value: unknown, projectId: string): Workflow {
  if (typeof value !== 'object' || value === null) return createDemoWorkflow(projectId);
  const candidate = value as Partial<Workflow>;
  if (
    candidate.schemaVersion !== 1 ||
    typeof candidate.id !== 'string' ||
    typeof candidate.name !== 'string' ||
    !Array.isArray(candidate.steps)
  ) {
    return createDemoWorkflow(projectId);
  }

  const steps = candidate.steps
    .filter((step): step is WorkflowStep => typeof step === 'object' && step !== null && typeof (step as WorkflowStep).id === 'string')
    .map((step) => ({
      ...step,
      title: typeof step.title === 'string' ? step.title : 'Untitled Step',
      kind: step.kind || 'click',
      timeoutMs: typeof step.timeoutMs === 'number' && Number.isFinite(step.timeoutMs) ? step.timeoutMs : 5000,
      maxAttempts: typeof step.maxAttempts === 'number' && Number.isFinite(step.maxAttempts) ? step.maxAttempts : 1,
    }));

  return { schemaVersion: 1, id: candidate.id, name: candidate.name, steps };
}

function readTarget(value: unknown): ProjectTarget {
  if (typeof value !== 'object' || value === null) return createApplicationTarget();
  const target = value as Partial<ProjectTarget>;
  if (target.kind !== 'configured-application') return createApplicationTarget();
  return {
    kind: target.kind,
    executablePath: typeof target.executablePath === 'string' ? target.executablePath.slice(0, 1024) : '',
    windowTitleContains: typeof target.windowTitleContains === 'string' ? target.windowTitleContains.slice(0, 200) : '',
    targetWindowId: typeof target.targetWindowId === 'number' ? target.targetWindowId : null,
  };
}

function readCredentialReferences(value: unknown): CredentialReference[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(
      (reference): reference is CredentialReference =>
        typeof reference === 'object' &&
        reference !== null &&
        typeof reference.id === 'string' &&
        typeof reference.alias === 'string' &&
        typeof reference.vaultKey === 'string' &&
        typeof reference.enabled === 'boolean',
    )
    .map((reference) => ({
      ...reference,
      alias: reference.alias.slice(0, 100),
      vaultKey: reference.vaultKey.slice(0, 300),
    }));
}

function readAccounts(value: unknown): AccountProfile[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(
      (acc): acc is AccountProfile =>
        typeof acc === 'object' &&
        acc !== null &&
        typeof acc.id === 'string' &&
        typeof acc.name === 'string' &&
        typeof acc.username === 'string' &&
        typeof acc.credentialTarget === 'string' &&
        typeof acc.isActive === 'boolean',
    )
    .map((acc) => ({
      id: acc.id,
      name: acc.name.slice(0, 100),
      username: acc.username.slice(0, 100),
      credentialTarget: acc.credentialTarget.slice(0, 200),
      notes: typeof acc.notes === 'string' ? acc.notes.slice(0, 500) : undefined,
      isActive: acc.isActive,
      lastUsedAt: typeof acc.lastUsedAt === 'string' ? acc.lastUsedAt : undefined,
    }));
}

// Only non-sensitive project metadata is stored here.
// Never store raw passwords, private tokens, or full screen captures in localStorage.
export function loadProjects(): Project[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value: unknown = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value
      .filter(
        (project): project is Omit<Project, 'workflow' | 'target' | 'credentialRefs'> =>
          typeof project === 'object' &&
          project !== null &&
          typeof project.id === 'string' &&
          typeof project.name === 'string' &&
          typeof project.description === 'string' &&
          typeof project.createdAt === 'string',
      )
      .map((project) => ({
        ...project,
        workflow: readWorkflow((project as Partial<Project>).workflow, project.id),
        target: readTarget((project as Partial<Project>).target),
        credentialRefs: readCredentialReferences((project as Partial<Project>).credentialRefs),
        accounts: readAccounts((project as Partial<Project>).accounts),
        activeAccountId: typeof (project as Partial<Project>).activeAccountId === 'string'
          ? (project as Partial<Project>).activeAccountId
          : null,
      }));
  } catch {
    return [];
  }
}

function sanitizeStepForStorage(step: WorkflowStep): WorkflowStep {
  const sanitized = { ...step };
  // Never save secrets or passwords in plain text in local storage
  if (sanitized.actionType === 'type-secret' || sanitized.kind === 'type-secret') {
    delete sanitized.text;
  }
  // Cap base64 image snippet size to prevent full-screen capture bloat and protect privacy
  if (sanitized.locator?.imageBase64 && sanitized.locator.imageBase64.length > 131072) {
    sanitized.locator = {
      ...sanitized.locator,
      imageBase64: undefined,
    };
  }
  if (sanitized.expectedLocator?.imageBase64 && sanitized.expectedLocator.imageBase64.length > 131072) {
    sanitized.expectedLocator = {
      ...sanitized.expectedLocator,
      imageBase64: undefined,
    };
  }
  return sanitized;
}

export function saveProjects(projects: Project[]): void {
  try {
    const sanitizedProjects = projects.map((p) => ({
      ...p,
      workflow: {
        ...p.workflow,
        steps: p.workflow.steps.map(sanitizeStepForStorage),
      },
    }));
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(sanitizedProjects));
  } catch {
    // Storage may be disabled; app remains usable for the current session.
  }
}

export function exportProjectBundle(project: Project): string {
  const bundle: ProjectExportBundle = {
    schemaVersion: 1,
    exportedAt: new Date().toISOString(),
    mimicVersion: '0.1.0',
    project: {
      id: project.id,
      name: project.name,
      description: project.description,
      workflow: {
        ...project.workflow,
        steps: project.workflow.steps.map(sanitizeStepForStorage),
      },
      target: {
        kind: 'configured-application',
        executablePath: project.target.executablePath,
        windowTitleContains: project.target.windowTitleContains,
      },
      accounts: (project.accounts || []).map((acc) => ({
        id: acc.id,
        name: acc.name,
        username: acc.username,
        // Opaque vault reference label only, never plaintext
        credentialTarget: acc.credentialTarget,
        notes: acc.notes,
        isActive: acc.isActive,
      })),
    },
  };
  return JSON.stringify(bundle, null, 2);
}

export interface ImportResult {
  project: Project;
  needsRemap: boolean;
  needsVaultSetup: boolean;
}

export function importProjectBundle(jsonString: string): ImportResult {
  const parsed = JSON.parse(jsonString) as unknown;
  if (typeof parsed !== 'object' || parsed === null) {
    throw new Error('Invalid project file format (not a JSON object)');
  }

  const bundle = parsed as Partial<ProjectExportBundle>;
  if (bundle.schemaVersion !== 1 || !bundle.project) {
    throw new Error('Unsupported project export schema version');
  }

  const rawProj = bundle.project;
  const newProjectId = crypto.randomUUID();
  const rawTarget = rawProj.target || { kind: 'configured-application', executablePath: '', windowTitleContains: '' };

  const importedAccounts: AccountProfile[] = (rawProj.accounts || []).map((acc) => ({
    id: crypto.randomUUID(),
    name: acc.name,
    username: acc.username,
    credentialTarget: `MIMIC:${rawProj.name?.toLowerCase().replace(/[^a-z0-9]/g, '_') || 'app'}:${acc.username}`,
    notes: acc.notes,
    isActive: acc.isActive ?? true,
    requiresVaultSetup: true, // Imported from another machine, needs password entry into local vault
  }));

  const project: Project = {
    id: newProjectId,
    name: rawProj.name ? `${rawProj.name} (Imported)` : 'Imported Project',
    description: rawProj.description || '',
    createdAt: new Date().toISOString(),
    workflow: readWorkflow(rawProj.workflow, newProjectId),
    target: {
      kind: 'configured-application',
      executablePath: rawTarget.executablePath || '',
      windowTitleContains: rawTarget.windowTitleContains || '',
      targetWindowId: null,
    },
    credentialRefs: [],
    accounts: importedAccounts,
    activeAccountId: importedAccounts[0]?.id ?? null,
  };

  const needsRemap = !project.target.executablePath || project.target.executablePath.length > 0;
  const needsVaultSetup = importedAccounts.length > 0;

  return { project, needsRemap, needsVaultSetup };
}
