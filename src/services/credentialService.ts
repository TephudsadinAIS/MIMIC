import { invoke } from '@tauri-apps/api/core';

export interface CredentialSummary {
  targetName: string;
  username: string;
  lastUpdated: string;
}

export interface CredentialEntry {
  targetName: string;
  username: string;
  secret: string;
}

function isDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function saveCredential(targetName: string, username: string, secret: string): Promise<void> {
  if (!isDesktop()) {
    // In mock/web environment, store securely in memory or session
    sessionStorage.setItem(`mimic_vault_${targetName}`, JSON.stringify({ targetName, username, secret }));
    return;
  }
  await invoke('credential_save', { targetName, username, secret });
}

export async function getCredential(targetName: string): Promise<CredentialEntry> {
  if (!isDesktop()) {
    const raw = sessionStorage.getItem(`mimic_vault_${targetName}`);
    if (!raw) throw new Error(`Credential '${targetName}' not found in mock vault`);
    return JSON.parse(raw) as CredentialEntry;
  }
  return await invoke<CredentialEntry>('credential_get', { targetName });
}

export async function deleteCredential(targetName: string): Promise<void> {
  if (!isDesktop()) {
    sessionStorage.removeItem(`mimic_vault_${targetName}`);
    return;
  }
  await invoke('credential_delete', { targetName });
}

export async function listCredentials(): Promise<CredentialSummary[]> {
  if (!isDesktop()) {
    const list: CredentialSummary[] = [];
    for (let i = 0; i < sessionStorage.length; i++) {
      const key = sessionStorage.key(i);
      if (key?.startsWith('mimic_vault_')) {
        const item = JSON.parse(sessionStorage.getItem(key) || '{}') as CredentialEntry;
        list.push({
          targetName: item.targetName,
          username: item.username,
          lastUpdated: new Date().toLocaleTimeString(),
        });
      }
    }
    return list;
  }
  return await invoke<CredentialSummary[]>('credential_list');
}
