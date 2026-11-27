import { JinshuView } from '@liulidesign/jinshu/JinshuView';
import type { JinshuDocument, JinshuProgress } from '@liulidesign/jinshu/schema';

// Test harness: the reader with optional copy and resume.
export function render({
  data,
  actions,
}: {
  data: { doc: JinshuDocument; withCopy?: boolean; resume?: JinshuProgress };
  actions: { copy: (code: string) => void };
}) {
  return (
    <page>
      <scroll className="grow">
        <JinshuView
          doc={data.doc}
          resume={data.resume}
          onCopyCode={data.withCopy ? code => actions.copy(code) : undefined}
          data-testid="reader"
        />
      </scroll>
    </page>
  );
}
