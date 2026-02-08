import { registerViewMethodHandler } from '../bridge';
import { showActionSheet } from './action-sheet';

export function registerUIHandlers(): void {
  registerViewMethodHandler('ui.showActionSheet', showActionSheet);
}
