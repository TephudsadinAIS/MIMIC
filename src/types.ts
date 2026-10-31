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

export type ProjectTargetKind = 'configured-application';

export interface ProjectTarget {
  kind: ProjectTargetKind;
  executablePath: string;
  windowTitleContains: string;
  targetWindowId?: number | null;
}

// A vault key is only an opaque reference. It must never contain the secret itself.
export interface CredentialReference {
  id: string;
  alias: string;
  vaultKey: string;
  enabled: boolean;
}

export type DemoState = 'login' | 'server' | 'ready';

export type ActionType =
  | 'click'
  | 'double-click'
  | 'type-text'
  | 'type-secret'
  | 'press-key'
  | 'wait-visible'
  | 'wait-hidden'
  | 'verify-state'
  | 'launch-app'
  | 'wait-for-state'
  | 'advance';

export type WorkflowStepKind = ActionType;

export interface Locator {
  kind: 'uia' | 'template' | 'window-title' | 'relative-point';
  automationId?: string;
  name?: string;
  controlType?: string;
  className?: string;
  imageBase64?: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  referenceWidth?: number;
  referenceHeight?: number;
  threshold?: number;
  value?: string;
}

export interface ExpectedResultConfig {
  kind: 'state' | 'element-visible' | 'element-hidden' | 'window-title';
  expectedState?: DemoState | string;
  locator?: Locator;
  timeoutMs: number;
}

export interface WorkflowStep {
  id: string;
  title: string;
  kind: WorkflowStepKind;
  actionType?: ActionType;
  expectedState?: DemoState | string;
  expectedResult?: ExpectedResultConfig;
  timeoutMs: number;
  maxAttempts: number;
  locator?: Locator;
  expectedLocator?: Locator;
  credentialRefId?: string;
  text?: string;
  secretRef?: string;
  key?: string;
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
  status?: 'info' | 'success' | 'warning' | 'error';
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
  exePath?: string;
  rect?: [number, number, number, number];
}

export interface AutomationRunResult {
  status: 'verified' | 'completed' | 'stopped';
  message: string;
  targetWindowId: number;
}

export interface StepExecutionResult {
  success: boolean;
  status: 'verified' | 'completed' | 'failed' | 'stopped';
  message: string;
  targetWindowId: number;
  attempts: number;
  durationMs: number;
}

export interface SelectOnScreenResult {
  cancelled: boolean;
  mode: 'uia' | 'region' | 'cancelled';
  x: number;
  y: number;
  width: number;
  height: number;
  windowId: number;
  windowTitle: string;
  elementName?: string;
  controlType?: string;
  automationId?: string;
  className?: string;
  imageBase64?: string;
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
  | { phase: 'disabled' }
  | { phase: 'browser' };
