// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Countdown, Inline } from '../../core/src/Timer/Timer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a timer to show a length of time that changes: a countdown, a recording."}>
        <Countdown />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Small and medium, in a line of text; hours appear from an hour on"}>
          <Inline />
        </Stage>
      </PageSection>
    </>
  );
}
