// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Services, Tones, Hues, DotOnly } from '../../core/src/StatusDot/StatusDot.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a status dot to say how something stands, in a row or a column of like things."}>
        <Services />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Success, warning, danger, info and accent (one colour where the accent is already a blue), and neutral as a ring"}>
          <Tones />
        </Stage>
        <Stage caption={"A hue for a category: which calendar an event is on"}>
          <Hues />
        </Stage>
        <Stage caption={"The dot alone, named by its word"}>
          <DotOnly />
        </Stage>
      </PageSection>
    </>
  );
}
