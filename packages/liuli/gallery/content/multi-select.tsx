// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Labels, Empty, WithIconsAndDescriptions, StateDisabled } from '../../core/src/MultiSelect/MultiSelect.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a multi-select to choose several things from a long list."}>
        <Labels />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Empty and required, with an error"}>
          <Empty />
        </Stage>
        <Stage caption={"An icon and a second line in the list; the token keeps the name"}>
          <WithIconsAndDescriptions />
        </Stage>
        <Stage caption={"Disabled, with the reason"}>
          <StateDisabled />
        </Stage>
      </PageSection>
    </>
  );
}
