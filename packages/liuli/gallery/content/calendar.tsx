// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Day, Stay, Weekdays, Chinese } from '../../core/src/Calendar/Calendar.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a calendar where people choose a day, or a span of days, by looking at a month."}>
        <Day />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A span: the first press starts it, the second ends it; past days cannot be chosen"}>
          <Stay />
        </Stage>
        <Stage caption={"Days the app rules out are dimmed"}>
          <Weekdays />
        </Stage>
        <Stage caption={"Words and week start from the locale"}>
          <Chinese />
        </Stage>
      </PageSection>
    </>
  );
}
