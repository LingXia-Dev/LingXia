// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as CheckboxInputSpecimen } from '../../core/src/CheckboxInput/CheckboxInput.examples';
import { Specimen as RadioGroupSpecimen } from '../../core/src/RadioGroup/RadioGroup.examples';
import { Specimen as SwitchSpecimen } from '../../core/src/Switch/Switch.examples';
import { Specimen as SelectSpecimen } from '../../core/src/Select/Select.examples';
import { Specimen as SegmentedControlSpecimen } from '../../core/src/SegmentedControl/SegmentedControl.examples';
import { Specimen as CheckboxListSpecimen } from '../../core/src/CheckboxList/CheckboxList.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Controls for choosing among options: one, several, or on and off."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"CheckboxInput"}
          tagline={"One on/off choice"}
          onOpen={() => onOpen("checkbox-input")}
          testId={"open-checkbox-input"}
        >
          <CheckboxInputSpecimen />
        </Tile>
        <Tile
          name={"RadioGroup"}
          tagline={"One of a few"}
          onOpen={() => onOpen("radio-group")}
          testId={"open-radio-group"}
        >
          <RadioGroupSpecimen />
        </Tile>
        <Tile
          name={"Switch"}
          tagline={"On or off, at once"}
          onOpen={() => onOpen("switch")}
          testId={"open-switch"}
        >
          <SwitchSpecimen />
        </Tile>
        <Tile
          name={"Select"}
          tagline={"One of many"}
          onOpen={() => onOpen("select")}
          testId={"open-select"}
        >
          <SelectSpecimen />
        </Tile>
        <Tile
          name={"SegmentedControl"}
          tagline={"One of a few"}
          onOpen={() => onOpen("segmented-control")}
          testId={"open-segmented-control"}
        >
          <SegmentedControlSpecimen />
        </Tile>
        <Tile
          name={"CheckboxList"}
          tagline={"Choose any number"}
          onOpen={() => onOpen("checkbox-list")}
          testId={"open-checkbox-list"}
        >
          <CheckboxListSpecimen />
        </Tile>
      </Category>
    </>
  );
}
