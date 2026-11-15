// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as TextInputSpecimen } from '../../core/src/TextInput/TextInput.examples';
import { Specimen as TextAreaSpecimen } from '../../core/src/TextArea/TextArea.examples';
import { Specimen as DateInputSpecimen } from '../../core/src/DateInput/DateInput.examples';
import { Specimen as TimeInputSpecimen } from '../../core/src/TimeInput/TimeInput.examples';
import { Specimen as SliderSpecimen } from '../../core/src/Slider/Slider.examples';
import { Specimen as FieldSpecimen } from '../../core/src/Field/Field.examples';
import { Specimen as SearchFieldSpecimen } from '../../core/src/SearchField/SearchField.examples';
import { Specimen as NumberInputSpecimen } from '../../core/src/NumberInput/NumberInput.examples';
import { Specimen as CalendarSpecimen } from '../../core/src/Calendar/Calendar.examples';
import { Specimen as DateRangeInputSpecimen } from '../../core/src/DateRangeInput/DateRangeInput.examples';
import { Specimen as FormLayoutSpecimen } from '../../core/src/FormLayout/FormLayout.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Fields a person types into or sets: text, dates, times and amounts."}</text>
      <Category title={"11 components"}>
        <Tile
          name={"TextInput"}
          tagline={"One line of text"}
          onOpen={() => onOpen("text-input")}
          testId={"open-text-input"}
        >
          <TextInputSpecimen />
        </Tile>
        <Tile
          name={"TextArea"}
          tagline={"Several lines"}
          onOpen={() => onOpen("text-area")}
          testId={"open-text-area"}
        >
          <TextAreaSpecimen />
        </Tile>
        <Tile
          name={"DateInput"}
          tagline={"One day"}
          onOpen={() => onOpen("date-input")}
          testId={"open-date-input"}
        >
          <DateInputSpecimen />
        </Tile>
        <Tile
          name={"TimeInput"}
          tagline={"A time of day"}
          onOpen={() => onOpen("time-input")}
          testId={"open-time-input"}
        >
          <TimeInputSpecimen />
        </Tile>
        <Tile
          name={"Slider"}
          tagline={"A value on a track"}
          onOpen={() => onOpen("slider")}
          testId={"open-slider"}
        >
          <SliderSpecimen />
        </Tile>
        <Tile
          name={"Field"}
          tagline={"Label, help, status"}
          onOpen={() => onOpen("field")}
          testId={"open-field"}
        >
          <FieldSpecimen />
        </Tile>
        <Tile
          name={"SearchField"}
          tagline={"Find in a list"}
          onOpen={() => onOpen("search-field")}
          testId={"open-search-field"}
        >
          <SearchFieldSpecimen />
        </Tile>
        <Tile
          name={"NumberInput"}
          tagline={"A number, by steps"}
          onOpen={() => onOpen("number-input")}
          testId={"open-number-input"}
        >
          <NumberInputSpecimen />
        </Tile>
        <Tile
          name={"Calendar"}
          tagline={"A month of days"}
          onOpen={() => onOpen("calendar")}
          testId={"open-calendar"}
        >
          <CalendarSpecimen />
        </Tile>
        <Tile
          name={"DateRangeInput"}
          tagline={"From a day to a day"}
          onOpen={() => onOpen("date-range-input")}
          testId={"open-date-range-input"}
        >
          <DateRangeInputSpecimen />
        </Tile>
        <Tile
          name={"FormLayout"}
          tagline={"Fields in a rhythm"}
          isWide
          onOpen={() => onOpen("form-layout")}
          testId={"open-form-layout"}
        >
          <FormLayoutSpecimen />
        </Tile>
      </Category>
    </>
  );
}
