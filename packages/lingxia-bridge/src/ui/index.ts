import { registerViewMethodHandler } from '../bridge';
import { dismissActionSheet, showActionSheet } from './action-sheet';
import { dismissModal, showModal } from './modal';
import { showToast, hideToast } from './toast';

export function registerUIHandlers(): void {
  registerViewMethodHandler('ui.showActionSheet', showActionSheet);
  registerViewMethodHandler('ui.showModal', showModal);
  registerViewMethodHandler('ui.showToast', showToast);
  registerViewMethodHandler('ui.hideToast', hideToast);
  registerViewMethodHandler('ui.dismissDialog', () => {
    dismissModal();
    dismissActionSheet();
    return Promise.resolve();
  });
}
