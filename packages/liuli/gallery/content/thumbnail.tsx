// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Grid, Strip, Sizes, InRows } from '../../core/src/Thumbnail/Thumbnail.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a thumbnail to stand for a picture, a file or an album."}>
        <Grid />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A strip, with the one in view ringed"}>
          <Strip />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small (48), medium (72), large (96), and large at 4:3"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"At the start of a row"}>
          <InRows />
        </Stage>
      </PageSection>
    </>
  );
}
