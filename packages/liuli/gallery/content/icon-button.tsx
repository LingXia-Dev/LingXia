// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Toolbar, Variants, Sizes, StateDefault, StateBusy, StateDisabled, InRows } from '../../core/src/IconButton/IconButton.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an icon button for an action people know by its icon, where a word would crowd the row."}>
        <Toolbar />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Primary, secondary, quiet and destructive; quiet by default"}>
          <Variants />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"32, 40 and 48 points square"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Default"}>
            <StateDefault />
          </Stage>
          <Stage caption={"While busy"}>
            <StateBusy />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"At the end of a row, named with the row it acts on"}>
          <InRows />
        </Stage>
      </PageSection>
    </>
  );
}
