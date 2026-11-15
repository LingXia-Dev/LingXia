// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Notifications, AtLeastOne, WithUnavailable, Sizes, StateDisabled } from '../../core/src/CheckboxList/CheckboxList.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a checkbox list when a question has a few answers and any number of them can be chosen."}>
        <Notifications />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A rule about the group, shown while the choice breaks it"}>
          <AtLeastOne />
        </Stage>
        <Stage caption={"An option that cannot be chosen stays in the list, with its reason"}>
          <WithUnavailable />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
