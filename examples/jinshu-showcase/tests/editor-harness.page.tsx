import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';
import type { AnyJinshuOp, JinshuDocument } from '@liulidesign/jinshu/schema';

// Test harness: the editor in controlled mode; the test applies emitted ops and re-renders.
export function render({ data, actions }: { data: { doc: JinshuDocument }; actions: { ops: (ops: readonly AnyJinshuOp[]) => void } }) {
  return (
    <page>
      <scroll className="grow">
        <JinshuEditor doc={data.doc} onOps={ops => actions.ops(ops)} hasLeaveGuard={false} data-testid="editor" />
      </scroll>
    </page>
  );
}
