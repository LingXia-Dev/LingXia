import type { Lx, PickedFile } from "../src/index.js";

declare const lx: Lx;

// Reading the payload without checking `canceled` is a compile error, for
// every dismissable API.
async function payloadNeedsTheCheck(): Promise<void> {
  // @ts-expect-error showActionSheet may have been dismissed
  (await lx.ui.actionSheet({ items: [{ id: "a", label: "A" }] })).id;
  // @ts-expect-error pick.file may have been dismissed
  (await lx.pick.file()).file;
  // @ts-expect-error pick.capture may have been dismissed
  (await lx.pick.capture({ type: "image" })).file;
  // @ts-expect-error pick.directory may have been dismissed
  (await lx.pick.directory()).directory;
  // @ts-expect-error pick.media may have been dismissed
  (await lx.pick.media()).files;
  // @ts-expect-error pick.code may have been dismissed
  (await lx.pick.code()).value;
}

// `status: 'ok'` narrows the payload with no cast and no non-null assertion.
async function checkedResultsNarrow(): Promise<string> {
  const sheet = await lx.ui.actionSheet({ items: [{ id: "a", label: "A" }, { id: "b", label: "B" }] });
  const index: string = (sheet.status === 'canceled') ? "" : sheet.id;

  const file = await lx.pick.file();
  const only: PickedFile | null = (file.status === 'canceled') ? null : file.file;
  const shot = await lx.pick.capture({ type: "video", facing: "front", maxDurationMs: 10_000 });
  const shotPath: string = (shot.status === 'canceled') ? "" : shot.file.path;
  const files = await lx.pick.files();
  const paths: string[] = (files.status === 'canceled') ? [] : files.files.map(f => f.path);
  const nonEmpty: readonly [PickedFile, ...PickedFile[]] | null = (files.status === 'canceled') ? null : files.files;

  const directory = await lx.pick.directory();
  const directoryPath: string = (directory.status === 'canceled') ? "" : directory.directory.path;

  const media = await lx.pick.media();
  const first: string = (media.status === 'canceled') ? "" : media.files[0].path;
  const firstType: string = (media.status === 'canceled') ? "" : media.files[0].type;

  const scan = await lx.pick.code();
  const code: string = (scan.status === 'canceled') ? "" : scan.value;

  const confirmed: boolean = await lx.ui.confirm("ok?");

  return [
    index,
    only?.name,
    shotPath,
    paths.length,
    nonEmpty?.length,
    directoryPath,
    first,
    firstType,
    code,
    confirmed,
  ].join(",");
}

async function actionSheetHasNoSentinel(): Promise<void> {
  const sheet = await lx.ui.actionSheet({ items: [{ id: "a", label: "A" }] });
  // @ts-expect-error the successful branch carries `id`, not `tapIndex`
  sheet.tapIndex;
}

export type CancellationGate = [
  typeof payloadNeedsTheCheck,
  typeof checkedResultsNarrow,
  typeof actionSheetHasNoSentinel,
];

// Dialogs take a `signal`; a bare-string dialog has nowhere to put one.
async function dialogsTakeASignal(signal: AbortSignal): Promise<void> {
  await lx.ui.alert({ title: "Saved", signal });
  const confirmed: boolean = await lx.ui.confirm({ title: "Delete?", tone: "danger", signal });
  await lx.ui.actionSheet({ items: [{ id: "a", label: "A" }], signal });
  // @ts-expect-error a signal is an AbortSignal, not a boolean
  await lx.ui.confirm({ title: "Delete?", signal: true });
  void confirmed;
}
