// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Services, Tones, DotOnly } from '../../core/src/StatusDot/StatusDot.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a status dot to say how something stands, in a row or a column of like things."}>
        <Services />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Success, warning, danger, accent, and neutral as a ring"}>
          <Tones />
        </Stage>
        <Stage caption={"The dot alone, named by its word"}>
          <DotOnly />
        </Stage>
      </PageSection>
    </>
  );
}
