// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Reminder, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, QuietHours } from '../../../core/src/TimeInput/TimeInput.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a time field to take a time of day, in the user's own clock."}>
        <Reminder />
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
        <Stage caption={"Quiet hours"}>
          <QuietHours />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
