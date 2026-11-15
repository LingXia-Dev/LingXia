// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Goal, Tones, Sizes, InRows } from '../../core/src/CircularProgress/CircularProgress.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a ring for progress where a bar has no room."}>
        <Goal />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A tone for a meter"}>
          <Tones />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small (24), medium (48) and large (80)"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Uploads, each with its ring"}>
          <InRows />
        </Stage>
      </PageSection>
    </>
  );
}
