import { invoke } from '@tauri-apps/api/core';
import type {
  AutomationRunResult,
  DemoObservation,
  SelectOnScreenResult,
  StepExecutionResult,
  TargetWindow,
  WorkflowStep,
} from '../types';
import { isDesktop } from './localAi';

function requireDesktop(): void {
  if (!isDesktop()) throw new Error('Windows Automation ใช้ได้เฉพาะใน MIMIC Desktop บน Windows');
}

export async function listTargetWindows(): Promise<TargetWindow[]> {
  requireDesktop();
  return invoke<TargetWindow[]>('automation_list_windows');
}

export async function pickExecutable(): Promise<string | null> {
  requireDesktop();
  return invoke<string | null>('automation_pick_executable');
}

export async function launchApplication(executablePath: string): Promise<number> {
  requireDesktop();
  return invoke<number>('automation_launch_application', { executablePath });
}

export async function focusWindow(targetWindowId: number): Promise<void> {
  requireDesktop();
  return invoke<void>('automation_focus_window', { targetWindowId });
}

export async function startDemoTarget(): Promise<void> {
  requireDesktop();
  return invoke<void>('automation_start_demo');
}

export async function advanceDemoTarget(
  targetWindowId: number,
  permissionGranted: boolean,
): Promise<AutomationRunResult> {
  requireDesktop();
  return invoke<AutomationRunResult>('automation_advance_demo', { targetWindowId, permissionGranted });
}

export async function observeDemoTarget(targetWindowId: number): Promise<DemoObservation> {
  requireDesktop();
  return invoke<DemoObservation>('automation_observe_demo', { targetWindowId });
}

export async function selectOnScreen(targetWindowId?: number): Promise<SelectOnScreenResult> {
  requireDesktop();
  return invoke<SelectOnScreenResult>('automation_select_on_screen', { targetWindowId });
}

export async function executeStep(
  targetWindowId: number,
  step: WorkflowStep,
  permissionGranted: boolean = false,
  scope?: { targetProcessId?: number; targetExePath?: string; targetWindowTitle?: string },
): Promise<StepExecutionResult> {
  requireDesktop();
  return invoke<StepExecutionResult>('automation_execute_step', {
    targetWindowId,
    step: {
      ...step,
      permissionGranted,
      targetProcessId: scope?.targetProcessId ?? step.targetProcessId,
      targetExePath: scope?.targetExePath ?? step.targetExePath,
      targetWindowTitle: scope?.targetWindowTitle ?? step.targetWindowTitle,
    },
  });
}

export async function requestEmergencyStop(): Promise<AutomationRunResult> {
  requireDesktop();
  return invoke<AutomationRunResult>('automation_emergency_stop');
}

export async function resetEmergencyStop(): Promise<void> {
  requireDesktop();
  return invoke<void>('automation_reset_emergency_stop');
}
