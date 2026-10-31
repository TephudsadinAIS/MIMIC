import type { CredentialReference, DemoState, Project, ProjectTarget, Workflow, WorkflowStep } from '../types';

const STORAGE_KEY = 'mimic.project-metadata.v1';

export function createApplicationTarget(): ProjectTarget {
  return { kind: 'configured-application', executablePath: '', windowTitleContains: '', targetWindowId: null };
}

export function createDemoWorkflow(id: string): Workflow {
  const step = (
    suffix: string,
    title: string,
    kind: WorkflowStep['kind'],
    expectedState: DemoState,
    locatorName?: string,
  ): WorkflowStep => ({
    id: `${id}-${suffix}`,
    title,
    kind,
    actionType: kind,
    expectedState,
    timeoutMs: 10_000,
    maxAttempts: 2,
    locator: locatorName ? { kind: 'uia', name: locatorName, controlType: 'Button' } : undefined,
  });

  return {
    schemaVersion: 1,
    id: `${id}-workflow`,
    name: 'Application Workflow',
    steps: [
      step('wait-window', 'Wait for Login Window', 'wait-visible', 'login'),
      step('click-login', 'Click Login Button', 'click', 'server', 'Continue to Server'),
      step('select-server', 'Select Test Server', 'click', 'ready', 'Select Test Server'),
      step('verify-ready', 'Verify Game Ready', 'verify-state', 'ready'),
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
