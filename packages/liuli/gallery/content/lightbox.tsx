// Generated from the doc file by scripts/generate.mjs; do not edit.
import { FromGrid, Single } from '../../core/src/Lightbox/Lightbox.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a lightbox to look at pictures whole."}>
        <FromGrid />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"One picture: no count and no dots"}>
          <Single />
        </Stage>
      </PageSection>
    </>
  );
}
