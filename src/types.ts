export type View = 'home' | 'workspace' | 'teach' | 'skills' | 'settings';

export interface Project {
  id: string;
  name: string;
  description: string;
  createdAt: string;
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

export type ConnectionState =
  | { phase: 'checking' }
  | { phase: 'ready'; status: AiStatus }
  | { phase: 'error'; reason: string }
  | { phase: 'browser' };
