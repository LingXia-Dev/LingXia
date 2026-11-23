// Generated from the doc file by scripts/generate.mjs; do not edit.
import { LiveStreamSplit, DocumentSheetSplit } from '../../core/src/Split/Split.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use Split for a primary canvas and its companion pane: beside each other on wide screens, upper and lower on tabletop folds, stacked or in a BottomSheet on phones."} bare>
        <LiveStreamSplit />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Secondary aside presented as a BottomSheet on compact"} bare>
          <DocumentSheetSplit />
        </Stage>
      </PageSection>
    </>
  );
}
