// Generated from the doc file by scripts/generate.mjs; do not edit.
import { LiveStreamSplit, DocumentSheetSplit } from '../../core/src/Split/Split.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use Split for the main content and a companion beside it: side by side on a wide screen, one above the other on a half-open fold, stacked or in a sheet on a phone."} bare>
        <LiveStreamSplit />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Comments in a sheet on a phone"} bare>
          <DocumentSheetSplit />
        </Stage>
      </PageSection>
    </>
  );
}
