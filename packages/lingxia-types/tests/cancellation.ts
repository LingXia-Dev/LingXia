import type { Lx } from "../src/index.js";

declare const lx: Lx;

// Reading the payload without checking `canceled` is a compile error, for
// every dismissable API.
async function payloadNeedsTheCheck(): Promise<void> {
  // @ts-expect-error showActionSheet may have been dismissed
  (await lx.showActionSheet({ itemList: ["a"] })).index;
  // @ts-expect-error chooseFile may have been dismissed
  (await lx.chooseFile()).paths;
  // @ts-expect-error chooseDirectory may have been dismissed
  (await lx.chooseDirectory()).path;
  // @ts-expect-error chooseMedia may have been dismissed
  (await lx.chooseMedia()).entries;
  // @ts-expect-error scanCode may have been dismissed
  (await lx.scanCode()).scanResult;
}

// `canceled: false` narrows the payload with no cast and no non-null assertion.
async function checkedResultsNarrow(): Promise<string> {
  const sheet = await lx.showActionSheet({ itemList: ["a", "b"] });
  const index: number = sheet.canceled ? -1 : sheet.index;

  const file = await lx.chooseFile();
  const paths: string[] = file.canceled ? [] : file.paths;

  const directory = await lx.chooseDirectory();
  const directoryPath: string = directory.canceled ? "" : directory.path;

  const media = await lx.chooseMedia();
  const first: string = media.canceled ? "" : media.entries[0].tempFilePath;

  const scan = await lx.scanCode();
  const code: string = scan.canceled ? "" : scan.scanResult;

  const modal = await lx.showModal({ content: "ok?" });
  const confirmed = !modal.canceled;

  return [index, paths.length, directoryPath, first, code, confirmed].join(",");
}

// The modal outcome is one bit; the mutually exclusive boolean pair is gone.
async function modalHasNoBooleanPair(): Promise<void> {
  const modal = await lx.showModal({ content: "ok?" });
  // @ts-expect-error confirm no longer exists
  modal.confirm;
  // @ts-expect-error cancel no longer exists
  modal.cancel;
}

async function actionSheetHasNoSentinel(): Promise<void> {
  const sheet = await lx.showActionSheet({ itemList: ["a"] });
  // @ts-expect-error tapIndex is replaced by index on the non-canceled branch
  sheet.tapIndex;
}

export type CancellationGate = [
  typeof payloadNeedsTheCheck,
  typeof checkedResultsNarrow,
  typeof modalHasNoBooleanPair,
  typeof actionSheetHasNoSentinel,
];
