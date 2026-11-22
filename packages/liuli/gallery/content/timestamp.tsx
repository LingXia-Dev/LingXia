// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Activity, Formats } from '../../core/src/Timestamp/Timestamp.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a timestamp to say when something happened."}>
        <Activity />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Relative, date, time, and date with time"}>
          <Formats />
        </Stage>
      </PageSection>
    </>
  );
}
