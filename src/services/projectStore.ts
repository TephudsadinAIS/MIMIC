import type { CredentialReference, DemoState, Project, ProjectTarget, Workflow, WorkflowStep } from '../types';

const STORAGE_KEY = 'mimic.project-metadata.v1';

export function createControlledDemoTarget(): ProjectTarget {
  return { kind: 'controlled-demo', executablePath: '', windowTitleContains: 'MIMIC Automation Demo' };
}

export function createDemoWorkflow(id: string): Workflow {
  const step = (suffix: string, title: string, kind: WorkflowStep['kind'], expectedState: DemoState): WorkflowStep => ({
    id: `${id}-${suffix}`,
    title,
    kind,
    expectedState,
    timeoutMs: 10_000,
    maxAttempts: 1,
  });
  return {
    schemaVersion: 1,
    id: `${id}-workflow`,
    name: 'Controlled Demo workflow',
    steps: [
      step('wait-login', 'Wait for Login', 'wait-for-state', 'login'),
      step('advance-server', 'Continue to Server', 'advance', 'server'),
      step('wait-server', 'Wait for Server selection', 'wait-for-state', 'server'),
      step('advance-ready', 'Select Test Server', 'advance', 'ready'),
    ],
  };
}

function isDemoState(value: unknown): value is DemoState {
  return value === 'login' || value === 'server' || value === 'ready';
}

function readWorkflow(value: unknown, projectId: string): Workflow {
  if (typeof value !== 'object' || value === null) return createDemoWorkflow(projectId);
  const candidate = value as Partial<Workflow>;
  if (candidate.schemaVersion !== 1 || typeof candidate.id !== 'string' || typeof candidate.name !== 'string' || !Array.isArray(candidate.steps)) {
    return createDemoWorkflow(projectId);
  }
  const steps = candidate.steps.filter((step): step is WorkflowStep =>
    typeof step === 'object' && step !== null &&
    typeof step.id === 'string' && typeof step.title === 'string' &&
    (step.kind === 'wait-for-state' || step.kind === 'advance' || step.kind === 'wait-for-window' || step.kind === 'click-uia' || step.kind === 'click-template' || step.kind === 'type-secret') &&
    isDemoState(step.expectedState) &&
    typeof step.timeoutMs === 'number' && Number.isFinite(step.timeoutMs) &&
    typeof step.maxAttempts === 'number' && Number.isFinite(step.maxAttempts),
  );
  return { schemaVersion: 1, id: candidate.id, name: candidate.name, steps };
}

function readTarget(value: unknown): ProjectTarget {
  if (typeof value !== 'object' || value === null) return createControlledDemoTarget();
  const target = value as Partial<ProjectTarget>;
  if (target.kind !== 'controlled-demo' && target.kind !== 'configured-application') return createControlledDemoTarget();
  return {
    kind: target.kind,
    executablePath: typeof target.executablePath === 'string' ? target.executablePath.slice(0, 1024) : '',
    windowTitleContains: typeof target.windowTitleContains === 'string' ? target.windowTitleContains.slice(0, 200) : '',
  };
}

function readCredentialReferences(value: unknown): CredentialReference[] {
  if (!Array.isArray(value)) return [];
  return value.filter((reference): reference is CredentialReference =>
    typeof reference === 'object' && reference !== null &&
    typeof reference.id === 'string' && typeof reference.alias === 'string' &&
    typeof reference.vaultKey === 'string' && typeof reference.enabled === 'boolean',
  ).map((reference) => ({ ...reference, alias: reference.alias.slice(0, 100), vaultKey: reference.vaultKey.slice(0, 300) }));
}

// Only non-sensitive project metadata is stored here as a transitional UI scaffold.
// Never store credentials, screenshots, events, or chat content in localStorage.
export function loadProjects(): Project[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value: unknown = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.filter((project): project is Omit<Project, 'workflow' | 'target' | 'credentialRefs'> =>
      typeof project === 'object' && project !== null &&
      typeof project.id === 'string' && typeof project.name === 'string' &&
      typeof project.description === 'string' && typeof project.createdAt === 'string',
    ).map((project) => ({
      ...project,
      workflow: readWorkflow((project as Partial<Project>).workflow, project.id),
      target: readTarget((project as Partial<Project>).target),
      credentialRefs: readCredentialReferences((project as Partial<Project>).credentialRefs),
    }));
  } catch {
    return [];
  }
}

export function saveProjects(projects: Project[]): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(projects));
  } catch {
    // Storage may be disabled; app remains usable for the current session.
  }
}
