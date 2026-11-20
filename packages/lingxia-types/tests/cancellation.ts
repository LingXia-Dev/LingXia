import type { PickedItem, PickedMediaEntry, Lx } from "../src/index.js";

declare const lx: Lx;

// Reading the payload without checking `canceled` is a compile error, for
// every dismissable API.
async function payloadNeedsTheCheck(): Promise<void> {
  // @ts-expect-error showActionSheet may have been dismissed
  (await lx.showActionSheet({ items: [{ id: "a", label: "A" }] })).id;
  // @ts-expect-error pickFiles may have been dismissed
  (await lx.pickFiles()).files;
  // @ts-expect-error pickDirectory may have been dismissed
  (await lx.pickDirectory()).uri;
  // @ts-expect-error pickMedia may have been dismissed
  (await lx.pickMedia()).entries;
  // @ts-expect-error scanCode may have been dismissed
  (await lx.scanCode()).scanResult;
}

// `status: 'ok'` narrows the payload with no cast and no non-null assertion.
async function checkedResultsNarrow(): Promise<string> {
  const sheet = await lx.showActionSheet({ items: [{ id: "a", label: "A" }, { id: "b", label: "B" }] });
  const index: string = (sheet.status === 'canceled') ? "" : sheet.id;

  const file = await lx.pickFiles();
  const uris: string[] = (file.status === 'canceled') ? [] : file.files.map((f) => f.uri);
  const nonEmptyFiles: [PickedItem, ...PickedItem[]] | null = (file.status === 'canceled') ? null : file.files;
  const single = await lx.pickFile();
  const name: string = (single.status === 'canceled') ? "" : single.name;

  const directory = await lx.pickDirectory();
  const directoryPath: string = (directory.status === 'canceled') ? "" : directory.uri;

  const media = await lx.pickMedia();
  const first: string = (media.status === 'canceled') ? "" : media.entries[0].uri;
  const nonEmptyMedia: [PickedMediaEntry, ...PickedMediaEntry[]] | null = (media.status === 'canceled')
    ? null
    : media.entries;

  const scan = await lx.scanCode();
  const code: string = (scan.status === 'canceled') ? "" : scan.scanResult;

  const modal = await lx.showModal({ content: "ok?" });
  const confirmed = modal.status !== 'canceled';

  return [
    index,
    uris.length,
    nonEmptyFiles?.length,
    name,
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
