/**
 * Runtime update APIs.
 */

export interface UpdateManager {
  applyUpdate(): void;
  onUpdateReady(callback: (info: UpdateReadyInfo) => void): void;
  onUpdateFailed(callback: (info: UpdateFailedInfo) => void): void;
}

export interface UpdateReadyInfo {
  version?: string;
  isForceUpdate?: boolean;
  releaseType?: "release" | "preview" | "developer" | string;
}

export interface UpdateFailedInfo extends UpdateReadyInfo {
  error?: string;
}
