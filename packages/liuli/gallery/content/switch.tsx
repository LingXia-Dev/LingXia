// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, InList, Sizes, StateOff, StateOn, StateDisabledOff, StateDisabledOn } from '../../core/src/Switch/Switch.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a switch for a setting that takes effect the moment it flips. On a form that is sent later, use a checkbox."}>
        <Basic />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"As rows in a list, beside an Item"}>
          <InList />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small (36 by 20) for dense rows, medium (44 by 26) by default"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Off"}>
            <StateOff />
          </Stage>
          <Stage caption={"On"}>
            <StateOn />
          </Stage>
          <Stage caption={"Disabled, off"}>
            <StateDisabledOff />
          </Stage>
          <Stage caption={"Disabled, on"}>
            <StateDisabledOn />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
