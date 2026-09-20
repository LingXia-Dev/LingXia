import type { ChosenMediaEntry, Lx } from "../src/index.js";

declare const lx: Lx;

// Reading the payload without checking `canceled` is a compile error, for
// every dismissable API.
async function payloadNeedsTheCheck(): Promise<void> {
  // @ts-expect-error showActionSheet may have been dismissed
  (await lx.showActionSheet({ items: [{ id: "a", label: "A" }] })).id;
  // @ts-expect-error chooseFile may have been dismissed
  (await lx.chooseFile()).paths;
  // @ts-expect-error chooseDirectory may have been dismissed
  (await lx.chooseDirectory()).path;
  // @ts-expect-error chooseMedia may have been dismissed
  (await lx.chooseMedia()).entries;
  // @ts-expect-error scanCode may have been dismissed
  (await lx.scanCode()).scanResult;
}

// `status: 'ok'` narrows the payload with no cast and no non-null assertion.
async function checkedResultsNarrow(): Promise<string> {
  const sheet = await lx.showActionSheet({ items: [{ id: "a", label: "A" }, { id: "b", label: "B" }] });
  const index: string = (sheet.status === 'canceled') ? "" : sheet.id;

  const file = await lx.chooseFile();
  const paths: string[] = (file.status === 'canceled') ? [] : file.paths;
  const nonEmptyPaths: [string, ...string[]] | null = (file.status === 'canceled') ? null : file.paths;

  const directory = await lx.chooseDirectory();
  const directoryPath: string = (directory.status === 'canceled') ? "" : directory.path;

  const media = await lx.chooseMedia();
  const first: string = (media.status === 'canceled') ? "" : media.entries[0].uri;
  const nonEmptyMedia: [ChosenMediaEntry, ...ChosenMediaEntry[]] | null = (media.status === 'canceled')
    ? null
    : media.entries;

  const scan = await lx.scanCode();
  const code: string = (scan.status === 'canceled') ? "" : scan.scanResult;

  const modal = await lx.showModal({ content: "ok?" });
  const confirmed = modal.status !== 'canceled';

  return [
    index,
    paths.length,
    nonEmptyPaths?.length,
    directoryPath,
    first,
    nonEmptyMedia?.length,
    code,
    confirmed,
  ].join(",");
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
  const sheet = await lx.showActionSheet({ items: [{ id: "a", label: "A" }] });
  // @ts-expect-error the successful branch carries `id`, not `tapIndex`
  sheet.tapIndex;
}

export type CancellationGate = [
  typeof payloadNeedsTheCheck,
  typeof checkedResultsNarrow,
  typeof modalHasNoBooleanPair,
  typeof actionSheetHasNoSentinel,
];
