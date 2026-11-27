import type { View } from '@lingxia/view';
import type { JinshuDocument } from '@liulidesign/jinshu/schema';
import { Library } from '../../content/library';
import type { DocRow } from '../../shared/agent';

type Actions = { open(args: { doc: string; mode?: 'read' | 'edit' }): Promise<void>; importClipboard(): Promise<void> };
type Data = { rows: DocRow[]; sections: Record<string, string[]>; excerpts: Record<string, JinshuDocument> };

export function render({ data, actions }: View<Actions, Data>) {
  return <Library rows={data.rows ?? []} sections={data.sections ?? {}} excerpts={data.excerpts ?? {}} onOpen={(doc, mode) => void actions.open({ doc, mode })} onImport={() => void actions.importClipboard()} />;
}
