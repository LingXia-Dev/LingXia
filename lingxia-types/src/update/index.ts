/**
 * Update APIs
 * Corresponds to: lingxia-logic/src/update.rs
 */

export interface UpdateManager {
  applyUpdate(): void;
  onUpdateReady(callback: () => void): void;
  onUpdateFailed(callback: () => void): void;
}
