// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Common, Colours, Labelled, Mirrored, Custom, Sizes, Everything } from '../../core/src/Icon/Icon.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an icon to make an action or a kind of thing quick to spot, beside the word that names it."}>
        <Common />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"An icon takes the colour of the text around it"}>
          <Colours />
        </Stage>
        <Stage caption={"Alone, it needs a label; beside text that says the same, it stays silent"}>
          <Labelled />
        </Stage>
        <Stage caption={"Left to right above, right to left below: the first three turn round, the search glass does not"}>
          <Mirrored />
        </Stage>
        <Stage caption={"An icon of your own, the default close, then a set that redraws close and keeps the rest"}>
          <Custom />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"16, 20 and 24"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"The whole set, by name"}>
          <Everything />
        </Stage>
      </PageSection>
    </>
  );
}
