// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Reminder, Required, StateDisabled, Narrow } from '../../core/src/DateTimeInput/DateTimeInput.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a date-time input when a day and a time are one answer."}>
        <Reminder />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Empty and required, with an error"}>
          <Required />
        </Stage>
        <Stage caption={"Disabled, with the reason"}>
          <StateDisabled />
        </Stage>
        <Stage caption={"In a narrow place the time goes under the date"}>
          <Narrow />
        </Stage>
      </PageSection>
    </>
  );
}
