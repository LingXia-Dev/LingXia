// Generated from the doc file by scripts/generate.mjs; do not edit.
import { MoveTo, Parts, Comments, Required } from '../../core/src/BottomSheet/BottomSheet.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a bottom sheet for a choice or a short task about the page under it."}>
        <MoveTo />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The grabber, the title, then the body"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"Expandable: opens halfway, drags up"}>
          <Comments />
        </Stage>
        <Stage caption={"A question that must be answered"}>
          <Required />
        </Stage>
      </PageSection>
    </>
  );
}
