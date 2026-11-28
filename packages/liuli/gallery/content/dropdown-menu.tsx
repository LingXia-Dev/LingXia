// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, Shortcuts, ShortcutsRtl, More, Value, Checkable, Grouped, Submenu, Repeatable, Data, Sizes, StateEnabled, StateDisabled, StateMixed, InList, InSentence } from '../../core/src/DropdownMenu/DropdownMenu.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a dropdown menu for a short list of actions that do not need to stay on screen."}>
        <Basic />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Shortcuts at the trailing edge, muted"}>
          <Shortcuts />
        </Stage>
        <Stage caption={"Right to left: the shortcuts mirror to the left edge"}>
          <ShortcutsRtl />
        </Stage>
        <Stage caption={"Icon only, at the end of a row"}>
          <More />
        </Stage>
        <Stage caption={"Showing the current choice"}>
          <Value />
        </Stage>
        <Stage caption={"Checkbox and radio items"}>
          <Checkable />
        </Stage>
        <Stage caption={"Named groups, for a menu with more than one kind of item"}>
          <Grouped />
        </Stage>
        <Stage caption={"A submenu at the side of its item, one level deep"}>
          <Submenu />
        </Stage>
        <Stage caption={"Rows that keep the menu open while a value beside it changes"}>
          <Repeatable />
        </Stage>
        <Stage caption={"The same rows given as data: a submenu, a group and a divider"}>
          <Data />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large triggers"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Default"}>
            <StateEnabled />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"State: Mixed: an item for a set, partly on"}>
            <StateMixed />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A menu on every row"}>
          <InList />
        </Stage>
        <Stage caption={"On a link in a sentence"}>
          <InSentence />
        </Stage>
      </PageSection>
    </>
  );
}
