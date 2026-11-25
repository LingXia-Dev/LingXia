import { useLocal } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { reportFailure, settleFlag } from './settle';

/**
 * An on/off control's async change, applied at once: the new value shows while the action runs, the
 * control reports busy and holds off further changes, and a failure puts the old value back and says
 * so under the control until the next change.
 */
export function useChangeAction(owner: string, label: string, apply: (value: boolean) => void, action: ((value: boolean) => Promise<unknown>) | undefined, failedMessage: string | undefined) {
  const pending = useLocal('change-pending', false);
  const failed = useLocal('change-failed', false);
  const change = async (next: boolean) => {
    if (pending.value) return;
    apply(next);
    if (!action) return;
    failed.set(false);
    pending.set(true);
    try {
      await action(next);
    } catch (error) {
      reportFailure(owner, label, error);
      apply(!next);
      settleFlag(failed, true);
    } finally {
      settleFlag(pending, false);
    }
  };
  return {
    isPending: pending.value,
    failure: failed.value ? failedMessage ?? liuliText('@liuli.actionFailed') : undefined,
    change,
  };
}
