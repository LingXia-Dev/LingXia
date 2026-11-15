// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Between, Labelled, Vertical } from '../../core/src/Divider/Divider.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a divider where two blocks would otherwise read as one."}>
        <Between />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A word on the line"}>
          <Labelled />
        </Stage>
        <Stage caption={"Upright, between items in a row"}>
          <Vertical />
        </Stage>
      </PageSection>
    </>
  );
}
