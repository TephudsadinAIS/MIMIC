export type View = 'home' | 'workspace' | 'teach' | 'skills' | 'settings';

export interface Project {
  id: string;
  name: string;
  description: string;
  createdAt: string;
  workflow: Workflow;
  target: ProjectTarget;
  credentialRefs: CredentialReference[];
}

export type ProjectTargetKind = 'controlled-demo' | 'configured-application';

export interface ProjectTarget {
  kind: ProjectTargetKind;
  executablePath: string;
  windowTitleContains: string;
}

// A vault key is only an opaque reference. It must never contain the secret itself.
export interface CredentialReference {
  id: string;
  alias: string;
  vaultKey: string;
  enabled: boolean;
}

export type DemoState = 'login' | 'server' | 'ready';
export type WorkflowStepKind =
  | 'wait-for-state'
  | 'advance'
  | 'wait-for-window'
  | 'click-uia'
  | 'click-template'
  | 'type-secret';

export type Locator =
  | { kind: 'window-title'; value: string }
  | { kind: 'uia'; automationId: string; name: string; controlType: string }
  | { kind: 'template'; assetId: string; threshold: number };

export interface WorkflowStep {
  id: string;
  title: string;
  kind: WorkflowStepKind;
  expectedState: DemoState;
  timeoutMs: number;
  maxAttempts: number;
  locator?: Locator;
  credentialRefId?: string;
}

export interface Workflow {
  schemaVersion: 1;
  id: string;
  name: string;
  steps: WorkflowStep[];
}

export type WorkflowRunPhase = 'idle' | 'running' | 'paused' | 'completed' | 'failed' | 'stopped';

export interface WorkflowRunEvent {
  stepId: string;
  message: string;
  at: string;
}

export interface AiStatus {
  connected: boolean;
  models: string[];
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

export interface TargetWindow {
  id: number;
  title: string;
  processId: number;
  isDemo: boolean;
}

export interface AutomationRunResult {
  status: 'verified' | 'completed' | 'stopped';
  message: string;
  targetWindowId: number;
}

export interface DemoObservation {
  state: DemoState;
  title: string;
  targetWindowId: number;
}

export type ConnectionState =
  | { phase: 'checking' }
  | { phase: 'ready'; status: AiStatus }
  | { phase: 'error'; reason: string }
  | { phase: 'browser' };
