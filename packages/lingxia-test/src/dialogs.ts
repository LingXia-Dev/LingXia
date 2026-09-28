import type {
  ActionSheetAnswer,
  DialogDriver,
  DialogUnwatchResult,
  LxAppDriver,
  ModalAnswer,
} from "@lingxia/types/automation";
import type { TestDialogs } from "./types.js";

/** The fixture surface the dialogs wrapper needs. */
export interface DialogsHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
}

function describeSheetAnswer(answer: ActionSheetAnswer): string {
  return "cancel" in answer ? "cancel" : `index ${String(answer.index)}`;
}

/**
 * `resolve` reads the raw driver lazily, inside each traced call: a host
 * without dialog watching fails that call, never the `t.app.dialogs` read.
 */
export function wrapDialogs(resolve: () => DialogDriver, host: DialogsHost): TestDialogs {
  return {
    toasts: () => host.act("dialogs.toasts", "", () => resolve().toasts()),
    modals: () => host.act("dialogs.modals", "", () => resolve().modals()),
    actionSheets: () => host.act("dialogs.actionSheets", "", () => resolve().actionSheets()),
    answerNextModal: (answer: ModalAnswer) =>
      host.act("dialogs.answerNextModal", answer?.confirm === true ? "confirm" : "cancel", () => {
        resolve().answerNextModal(answer);
      }),
    answerNextActionSheet: (answer: ActionSheetAnswer) =>
      host.act("dialogs.answerNextActionSheet", answer && typeof answer === "object" ? describeSheetAnswer(answer) : "", () => {
        resolve().answerNextActionSheet(answer);
      }),
  };
}

/** One spec's watch of the app under test's dialogs. */
export interface DialogWatch {
  /** Resolves the first dialog that found no answer; never resolves otherwise. */
  readonly unanswered: Promise<string>;
  /**
   * Stop watching, through `driver` (the app under test as selected now: a
   * profile switch reopens it); what the spec queued and no dialog used, or
   * `undefined` when nothing is left.
   */
  end(driver: LxAppDriver): Promise<string | undefined>;
}

/**
 * Watch the app's dialogs for the spec that starts now. `undefined` on a
 * host without dialog watching. A refused watch fails spec setup.
 */
export function watchDialogs(driver: LxAppDriver): DialogWatch | undefined {
  const dialogs = (driver as Partial<LxAppDriver>).dialogs;
  if (!dialogs || typeof dialogs.watch !== "function") return undefined;
  dialogs.watch();
  const watched = dialogs;
  let observationError: unknown;
  const unanswered = new Promise<string>((resolve) => {
    Promise.resolve()
      .then(() => watched.unanswered())
      .then((message) => { if (typeof message === "string") resolve(message); }, (error: unknown) => {
        observationError = error ?? new Error("dialog observation failed without an error value");
        resolve(`dialog observation failed: ${String(error)}`);
      });
  });
  return {
    unanswered,
    async end(driver) {
      const left: DialogUnwatchResult = await driver.dialogs.unwatch();
      if (observationError !== undefined) throw observationError;
      const parts: string[] = [];
      if (left.modalAnswers > 0) {
        parts.push(`${left.modalAnswers} modal answer${left.modalAnswers === 1 ? "" : "s"} (t.app.dialogs.answerNextModal)`);
      }
      if (left.actionSheetAnswers > 0) {
        parts.push(`${left.actionSheetAnswers} action sheet answer${left.actionSheetAnswers === 1 ? "" : "s"} (t.app.dialogs.answerNextActionSheet)`);
      }
      return parts.length === 0 ? undefined : `${parts.join(" and ")} queued but no dialog appeared to use ${left.modalAnswers + left.actionSheetAnswers === 1 ? "it" : "them"}`;
    },
  };
}
