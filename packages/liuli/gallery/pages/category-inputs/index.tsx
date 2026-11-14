// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as TextInputSpecimen } from '../../../core/src/TextInput/TextInput.examples';
import { Specimen as TextAreaSpecimen } from '../../../core/src/TextArea/TextArea.examples';
import { Specimen as DateInputSpecimen } from '../../../core/src/DateInput/DateInput.examples';
import { Specimen as TimeInputSpecimen } from '../../../core/src/TimeInput/TimeInput.examples';
import { Specimen as SliderSpecimen } from '../../../core/src/Slider/Slider.examples';
import { Specimen as FieldSpecimen } from '../../../core/src/Field/Field.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Fields a person types into or sets: text, dates, times and amounts."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"TextInput"}
          tagline={"One line of text"}
          onOpen={() => actions.open({ page: "text-input" })}
          testId={"open-text-input"}
        >
          <TextInputSpecimen />
        </Tile>
        <Tile
          name={"TextArea"}
          tagline={"Several lines"}
          onOpen={() => actions.open({ page: "text-area" })}
          testId={"open-text-area"}
        >
          <TextAreaSpecimen />
        </Tile>
        <Tile
          name={"DateInput"}
          tagline={"One day"}
          onOpen={() => actions.open({ page: "date-input" })}
          testId={"open-date-input"}
        >
          <DateInputSpecimen />
        </Tile>
        <Tile
          name={"TimeInput"}
          tagline={"A time of day"}
          onOpen={() => actions.open({ page: "time-input" })}
          testId={"open-time-input"}
        >
          <TimeInputSpecimen />
        </Tile>
        <Tile
          name={"Slider"}
          tagline={"A value on a track"}
          onOpen={() => actions.open({ page: "slider" })}
          testId={"open-slider"}
        >
          <SliderSpecimen />
        </Tile>
        <Tile
          name={"Field"}
          tagline={"Label, help, status"}
          onOpen={() => actions.open({ page: "field" })}
          testId={"open-field"}
        >
          <FieldSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
