// Generated from the doc file by scripts/generate.mjs; do not edit.
import { InARow, Tones, Solid, Hues, Sizes } from '../../core/src/Badge/Badge.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a badge to mark the status or kind of the thing beside it, in a word or two."}>
        <InARow />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Soft, in six tones"}>
          <Tones />
        </Stage>
        <Stage caption={"Solid, for the one label that must stand out"}>
          <Solid />
        </Stage>
        <Stage caption={"A category takes one of eight hues: a dot in the hue on its soft fill"}>
          <Hues />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium, with an icon"}>
          <Sizes />
        </Stage>
      </PageSection>
    </>
  );
}
