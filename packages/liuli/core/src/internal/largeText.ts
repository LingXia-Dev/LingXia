import { useView } from '@lingxia/view';

/** From this text scale a row that would squeeze its text stacks it instead, as iOS's accessibility sizes do. */
const stackScale = 1.5;

export function useLargeText(): boolean {
  return (useView().env.textScale ?? 1) >= stackScale;
}
