import { registerViewMethodHandler } from '../bridge';
import { showActionSheet } from './action-sheet';
import { showModal } from './modal';
import { showToast, hideToast } from './toast';

export function registerUIHandlers(): void {
  registerViewMethodHandler('ui.showActionSheet', showActionSheet);
  registerViewMethodHandler('ui.showModal', showModal);
  registerViewMethodHandler('ui.showToast', showToast);
  registerViewMethodHandler('ui.hideToast', hideToast);
}
