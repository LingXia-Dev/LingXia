import { useLocal } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { reportFailure, settleFlag } from './settle';

/**
 * An on/off control's async change, applied at once: the new value shows while the action runs, the
 * control reports busy and holds off further changes, and a failure puts the old value back and says
 * so under the control until the next change.
 */
export function useChangeAction(
  owner: string,
  label: string,
  apply: (value: boolean) => void,
  action: ((value: boolean) => Promise<unknown>) | undefined,
  failedMessage: string | undefined,
  /** Told the failure's words when one happens, and `null` at the next change: an Item row draws them. */
  report?: (message: string | null) => void,
) {
  const pending = useLocal('change-pending', false);
  const failed = useLocal('change-failed', false);
  const message = failedMessage ?? liuliText('@liuli.actionFailed');
  const change = async (next: boolean) => {
    if (pending.value) return;
    apply(next);
    if (!action) return;
    if (failed.value) report?.(null);
    failed.set(false);
    pending.set(true);
    try {
      await action(next);
    } catch (error) {
      reportFailure(owner, label, error);
      apply(!next);
      settleFlag(failed, true);
      report?.(message);
    } finally {
      settleFlag(pending, false);
    }
  };
  return {
    isPending: pending.value,
    failure: failed.value ? message : undefined,
    change,
  };
}
