// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Toolbar, Parts, Below, OnWords, InSentence } from '../../core/src/Tooltip/Tooltip.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a tooltip to name an icon button in words. Rest a pointer on the button, or long-press it on a phone."}>
        <Toolbar />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The hint"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"Opening below, where there is no room above"}>
          <Below />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"On words: wrapped in one plain button"}>
          <OnWords />
        </Stage>
        <Stage caption={"On a link in a sentence"}>
          <InSentence />
        </Stage>
      </PageSection>
    </>
  );
}
