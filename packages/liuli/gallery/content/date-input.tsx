// Generated from the doc file by scripts/generate.mjs; do not edit.
import { DueDate, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, Trip, Meeting } from '../../core/src/DateInput/DateInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a date field to take one day, picked from a month."}>
        <DueDate />
      </Lead>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid stack>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Filled"}>
            <StateFilled />
          </Stage>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A trip: leave, then return"}>
          <Trip />
        </Stage>
        <Stage caption={"A day and a time side by side in a narrow row; each keeps room for its whole value"}>
          <Meeting />
        </Stage>
      </PageSection>
    </>
  );
}
