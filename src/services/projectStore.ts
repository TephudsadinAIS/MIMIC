import type { Project } from '../types';

const STORAGE_KEY = 'mimic.project-metadata.v1';

// Only non-sensitive project metadata is stored here as a transitional UI scaffold.
// Never store credentials, screenshots, events, or chat content in localStorage.
export function loadProjects(): Project[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value: unknown = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.filter((project): project is Project =>
      typeof project === 'object' && project !== null &&
      typeof project.id === 'string' && typeof project.name === 'string' &&
      typeof project.description === 'string' && typeof project.createdAt === 'string'
    );
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
