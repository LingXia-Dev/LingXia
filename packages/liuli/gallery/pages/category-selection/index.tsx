// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as CheckboxInputSpecimen } from '../../../core/src/CheckboxInput/CheckboxInput.examples';
import { Specimen as RadioGroupSpecimen } from '../../../core/src/RadioGroup/RadioGroup.examples';
import { Specimen as SwitchSpecimen } from '../../../core/src/Switch/Switch.examples';
import { Specimen as SelectSpecimen } from '../../../core/src/Select/Select.examples';
import { Specimen as SegmentedControlSpecimen } from '../../../core/src/SegmentedControl/SegmentedControl.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Controls for choosing among options: one, several, or on and off."}</text>
      <Category title={"5 components"}>
        <Tile
          name={"CheckboxInput"}
          tagline={"One on/off choice"}
          onOpen={() => actions.open({ page: "checkbox-input" })}
          testId={"open-checkbox-input"}
        >
          <CheckboxInputSpecimen />
        </Tile>
        <Tile
          name={"RadioGroup"}
          tagline={"One of a few"}
          onOpen={() => actions.open({ page: "radio-group" })}
          testId={"open-radio-group"}
        >
          <RadioGroupSpecimen />
        </Tile>
        <Tile
          name={"Switch"}
          tagline={"On or off, at once"}
          onOpen={() => actions.open({ page: "switch" })}
          testId={"open-switch"}
        >
          <SwitchSpecimen />
        </Tile>
        <Tile
          name={"Select"}
          tagline={"One of many"}
          onOpen={() => actions.open({ page: "select" })}
          testId={"open-select"}
        >
          <SelectSpecimen />
        </Tile>
        <Tile
          name={"SegmentedControl"}
          tagline={"One of a few"}
          isWide
          onOpen={() => actions.open({ page: "segmented-control" })}
          testId={"open-segmented-control"}
        >
          <SegmentedControlSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
