import { invoke } from '@tauri-apps/api/core';
import type { AiStatus, ChatMessage } from '../types';

export function isDesktop(): boolean {
  return typeof window !== 'undefined' && Object.prototype.hasOwnProperty.call(window, '__TAURI_INTERNALS__');
}

// All AI calls pass through narrow native commands. No browser-side secrets or cloud fallback.
export async function checkLocalAi(): Promise<AiStatus> {
  if (!isDesktop()) {
    throw new Error('ใช้คำสั่ง npm run dev เพื่อเปิด MIMIC Desktop สำหรับเชื่อม LM Studio');
  }
  return invoke<AiStatus>('ai_status');
}

export async function chatWithLocalAi(model: string, messages: ChatMessage[]): Promise<string> {
  if (!isDesktop()) {
    throw new Error('ไม่สามารถส่งข้อความหา LM Studio จาก Browser Preview');
  }
  return invoke<string>('ai_chat', {
    model,
    messages: messages.map(({ role, content }) => ({ role, content })),
  });
}
