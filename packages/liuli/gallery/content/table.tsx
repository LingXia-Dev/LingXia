// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Invoices, Pressable, Sortable, SortControlled, SortStacked, Scrolling, Members, MembersColumns, StateEmpty } from '../../core/src/Table/Table.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a table for records that share the same fields, when people compare them across rows."} bare>
        <Invoices />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Rows that open a record, the open one selected; a cell drawn as a badge"} bare>
          <Pressable />
        </Stage>
        <Stage caption={"Headings that sort the rows, a second press the other way"} bare>
          <Sortable />
        </Stage>
        <Stage caption={"The order held by the page"} bare>
          <SortControlled />
        </Stage>
        <Stage caption={"Stacked records, sorted from one menu"} bare>
          <SortStacked />
        </Stage>
        <Stage caption={"Compact rows that scroll under the headings"} bare>
          <Scrolling />
        </Stage>
        <Stage caption={"Four columns: stacked where they do not fit, in columns where they do"} bare>
          <Members />
        </Stage>
        <Stage caption={"The same table held in columns; on a phone its text columns are too narrow"} bare>
          <MembersColumns />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"With no rows"} bare>
            <StateEmpty />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
