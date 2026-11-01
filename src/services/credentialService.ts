import { invoke } from '@tauri-apps/api/core';

export interface CredentialSummary {
  targetName: string;
  username: string;
  lastUpdated: string;
}

export interface CredentialMetadata {
  targetName: string;
  username: string;
  exists: boolean;
  lastUpdated: string;
}

function isDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function saveCredential(targetName: string, username: string, secret: string): Promise<void> {
  if (!isDesktop()) {
    // In mock/preview environment, only store non-sensitive metadata.
    // Plaintext secrets are strictly NEVER written to sessionStorage, localStorage, or cookies.
    sessionStorage.setItem(
      `mimic_vault_meta_${targetName}`,
      JSON.stringify({
        targetName,
        username,
        exists: true,
        lastUpdated: new Date().toLocaleTimeString(),
      }),
    );
    return;
  }
  await invoke('credential_save', { targetName, username, secret });
}

export async function checkCredential(targetName: string): Promise<CredentialMetadata> {
  if (!isDesktop()) {
    const raw = sessionStorage.getItem(`mimic_vault_meta_${targetName}`);
    if (!raw) throw new Error(`Credential '${targetName}' not found in mock vault`);
    return JSON.parse(raw) as CredentialMetadata;
  }
  return await invoke<CredentialMetadata>('credential_check', { targetName });
}

export async function deleteCredential(targetName: string): Promise<void> {
  if (!isDesktop()) {
    sessionStorage.removeItem(`mimic_vault_meta_${targetName}`);
    return;
  }
  await invoke('credential_delete', { targetName });
}

export async function listCredentials(): Promise<CredentialSummary[]> {
  if (!isDesktop()) {
    const list: CredentialSummary[] = [];
    for (let i = 0; i < sessionStorage.length; i++) {
      const key = sessionStorage.key(i);
      if (key?.startsWith('mimic_vault_meta_')) {
        const item = JSON.parse(sessionStorage.getItem(key) || '{}') as CredentialMetadata;
        list.push({
          targetName: item.targetName,
          username: item.username,
          lastUpdated: item.lastUpdated,
        });
      }
    }
    return list;
  }
  return await invoke<CredentialSummary[]>('credential_list');
}
