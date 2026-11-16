// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Delivery, Plans, Plain, Inline, Sizes, StateDefault, StateInvalid, StateDisabledOption, StateDisabled, Required } from '../../core/src/RadioGroup/RadioGroup.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use radios to pick one option from a short list when seeing every option helps the choice."}>
        <Delivery />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Cards: the whole card chooses"}>
          <Plans />
        </Stage>
        <Stage caption={"A plain list with help text"}>
          <Plain />
        </Stage>
        <Stage caption={"Horizontal, for two or three short options"}>
          <Inline />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Default"}>
            <StateDefault />
          </Stage>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"A disabled option"}>
            <StateDisabledOption />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A required question, after a failed send"}>
          <Required />
        </Stage>
      </PageSection>
    </>
  );
}
