import { invoke } from '@tauri-apps/api/core';
import type { AutomationRunResult, DemoObservation, TargetWindow } from '../types';
import { isDesktop } from './localAi';

function requireDesktop(): void {
  if (!isDesktop()) throw new Error('Windows Automation ใช้ได้เฉพาะใน MIMIC Desktop บน Windows');
}

export async function listTargetWindows(): Promise<TargetWindow[]> {
  requireDesktop();
  return invoke<TargetWindow[]>('automation_list_windows');
}

export async function startDemoTarget(): Promise<void> {
  requireDesktop();
  return invoke<void>('automation_start_demo');
}

export async function advanceDemoTarget(targetWindowId: number, permissionGranted: boolean): Promise<AutomationRunResult> {
  requireDesktop();
  return invoke<AutomationRunResult>('automation_advance_demo', { targetWindowId, permissionGranted });
}

export async function observeDemoTarget(targetWindowId: number): Promise<DemoObservation> {
  requireDesktop();
  return invoke<DemoObservation>('automation_observe_demo', { targetWindowId });
}

export async function requestEmergencyStop(): Promise<AutomationRunResult> {
  requireDesktop();
  return invoke<AutomationRunResult>('automation_emergency_stop');
}
