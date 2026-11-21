import type { Lx, PickedFile } from "../src/index.js";

declare const lx: Lx;

// Reading the payload without checking `canceled` is a compile error, for
// every dismissable API.
async function payloadNeedsTheCheck(): Promise<void> {
  // @ts-expect-error showActionSheet may have been dismissed
  (await lx.ui.actionSheet({ items: [{ id: "a", label: "A" }] })).id;
  // @ts-expect-error pick.file may have been dismissed
  (await lx.pick.file()).files;
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
  const only: readonly [PickedFile] | null = (file.status === 'canceled') ? null : file.files;
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
    only?.[0].name,
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
