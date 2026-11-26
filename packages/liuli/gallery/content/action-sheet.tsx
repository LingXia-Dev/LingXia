// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, Parts, WithMessage, Untitled } from '../../core/src/ActionSheet/ActionSheet.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an action sheet to ask what to do with one thing; Cancel is always there."}>
        <Basic />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The title, the actions, then Cancel apart"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"A question with its consequence spelled out"}>
          <WithMessage />
        </Stage>
        <Stage caption={"No drawn title"}>
          <Untitled />
        </Stage>
      </PageSection>
    </>
  );
}
