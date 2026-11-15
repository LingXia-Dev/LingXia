// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Actions, Attached, IconsAttached, Sizes, StateDisabled, Toolbar } from '../../core/src/ButtonGroup/ButtonGroup.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a button group for actions on the same thing, so they read and are reached as one set."}>
        <Actions />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Attached: actions of one kind share edges"}>
          <Attached />
        </Stage>
        <Stage caption={"Attached icon buttons"}>
          <IconsAttached />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"The group sets the size of its buttons"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Disabled, all at once"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A quiet group of icon buttons, with a labelled action apart"}>
          <Toolbar />
        </Stage>
      </PageSection>
    </>
  );
}
