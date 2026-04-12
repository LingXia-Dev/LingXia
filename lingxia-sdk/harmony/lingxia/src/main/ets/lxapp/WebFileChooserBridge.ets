import { hilog } from '@kit.PerformanceAnalysisKit';
import { onWebFileChooserRequested } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebFileChooser';

let nextRequestId = 1;
const pendingResults: Map<string, (selectedPaths: Array<string>) => void> = new Map();

export function requestWebFileChooser(
  webtag: string,
  completeSelection: (selectedPaths: Array<string>) => void,
  acceptTypes: Array<string>,
  mode: number,
  capture: boolean,
  sourceUrl: string
): boolean {
  const requestId = `${nextRequestId++}`;
  pendingResults.set(requestId, completeSelection);
  const acceptedJson = JSON.stringify(acceptTypes);
  const handled: boolean = onWebFileChooserRequested(
    requestId,
    webtag,
    sourceUrl,
    acceptedJson,
    mode === 1,
    mode === 2,
    capture
  );
  if (!handled) {
    pendingResults.delete(requestId);
  }
  return handled;
}

export function completeWebFileChooserRequest(requestId: string, selectedJson: string): boolean {
  const completeSelection: ((selectedPaths: Array<string>) => void) | undefined = pendingResults.get(requestId);
  pendingResults.delete(requestId);
  if (!completeSelection) {
    return false;
  }

  try {
    const parsed: Object = JSON.parse(selectedJson) as Object;
    const selected: Array<string> = Array.isArray(parsed) ? parsed as Array<string> : [];
    completeSelection(selected);
    return true;
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `completeWebFileChooserRequest parse failed: ${JSON.stringify(error)}`);
    completeSelection([]);
    return false;
  }
}
