// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Volume, Stepped, Range, InList, Marks, SaveOnRelease, Sizes, StateDefault, StateDisabled, DisabledWithReason, Position } from '../../core/src/Slider/Slider.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a slider when the value is easier to feel than to type: a volume, a size, a place in a recording."}>
        <Volume />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Whole steps, named in words"}>
          <Stepped />
        </Stage>
        <Stage caption={"A range: two thumbs that stop at each other, each with its own name"}>
          <Range />
        </Stage>
        <Stage caption={"As a row in a list, beside Item rows"}>
          <InList />
        </Stage>
        <Stage caption={"Marks: ticks under the track, labelled where it helps"}>
          <Marks />
        </Stage>
        <Stage caption={"Saving once, when the drag ends"}>
          <SaveOnRelease />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small for dense rows and media, medium by default"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Default"}>
            <StateDefault />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"State: Disabled, with the reason in place of the help text"}>
            <DisabledWithReason />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A position in a recording, with what has loaded"}>
          <Position />
        </Stage>
      </PageSection>
    </>
  );
}
