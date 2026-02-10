import { DomainValidationError } from "../domain/auth";
import { type WorkspaceRecord, workspaceFromStorage } from "../domain/chat";

const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";
const MAX_CACHE_BYTES = 131_072;

function canUseStorage(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.localStorage !== "undefined" &&
    typeof window.localStorage.getItem === "function" &&
    typeof window.localStorage.setItem === "function" &&
    typeof window.localStorage.removeItem === "function"
  );
}

export function loadWorkspaceCache(): WorkspaceRecord[] {
  if (!canUseStorage()) {
    return [];
  }
  const raw = window.localStorage.getItem(WORKSPACE_CACHE_KEY);
  if (!raw || raw.length > MAX_CACHE_BYTES) {
    return [];
  }
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.map((entry) => workspaceFromStorage(entry));
  } catch (error) {
    if (error instanceof DomainValidationError) {
      return [];
    }
    return [];
  }
}

export function saveWorkspaceCache(cache: WorkspaceRecord[]): void {
  if (!canUseStorage()) {
    return;
  }
  window.localStorage.setItem(WORKSPACE_CACHE_KEY, JSON.stringify(cache));
}

export function clearWorkspaceCache(): void {
  if (!canUseStorage()) {
    return;
  }
  window.localStorage.removeItem(WORKSPACE_CACHE_KEY);
}
