// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as BadgeSpecimen } from '../../../core/src/Badge/Badge.examples';
import { Specimen as ButtonSpecimen } from '../../../core/src/Button/Button.examples';
import { Specimen as CardSpecimen } from '../../../core/src/Card/Card.examples';
import { Specimen as CheckboxInputSpecimen } from '../../../core/src/CheckboxInput/CheckboxInput.examples';
import { Specimen as DialogSpecimen } from '../../../core/src/Dialog/Dialog.examples';
import { Specimen as DropdownMenuSpecimen } from '../../../core/src/DropdownMenu/DropdownMenu.examples';
import { Specimen as FieldSpecimen } from '../../../core/src/Field/Field.examples';
import { Specimen as ItemSpecimen } from '../../../core/src/Item/Item.examples';
import { Specimen as ProgressBarSpecimen } from '../../../core/src/ProgressBar/ProgressBar.examples';
import { Specimen as TextInputSpecimen } from '../../../core/src/TextInput/TextInput.examples';
import { Category, GalleryPage, Masthead, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <Masthead version={"0.1.0"} status={"Preview"} />
      <Category title={"Actions"}>
        <Tile
          name={"Button"}
          tagline={"Starts an action"}
          isWide
          onOpen={() => actions.open({ page: "button" })}
          testId={"open-button"}
        >
          <ButtonSpecimen />
        </Tile>
      </Category>
      <Category title={"Inputs"}>
        <Tile
          name={"TextInput"}
          tagline={"One line of text"}
          onOpen={() => actions.open({ page: "text-input" })}
          testId={"open-text-input"}
        >
          <TextInputSpecimen />
        </Tile>
        <Tile
          name={"CheckboxInput"}
          tagline={"One on/off choice"}
          onOpen={() => actions.open({ page: "checkbox-input" })}
          testId={"open-checkbox-input"}
        >
          <CheckboxInputSpecimen />
        </Tile>
        <Tile
          name={"Field"}
          tagline={"Label, help and status"}
          isWide
          onOpen={() => actions.open({ page: "field" })}
          testId={"open-field"}
        >
          <FieldSpecimen />
        </Tile>
      </Category>
      <Category title={"Feedback"}>
        <Tile
          name={"Badge"}
          tagline={"A short status label"}
          onOpen={() => actions.open({ page: "badge" })}
          testId={"open-badge"}
        >
          <BadgeSpecimen />
        </Tile>
        <Tile
          name={"ProgressBar"}
          tagline={"Progress, or a wait"}
          onOpen={() => actions.open({ page: "progress-bar" })}
          testId={"open-progress-bar"}
        >
          <ProgressBarSpecimen />
        </Tile>
      </Category>
      <Category title={"Overlays"}>
        <Tile
          name={"Dialog"}
          tagline={"A decision over the page"}
          onOpen={() => actions.open({ page: "dialog" })}
          testId={"open-dialog"}
        >
          <DialogSpecimen />
        </Tile>
        <Tile
          name={"DropdownMenu"}
          tagline={"A menu of actions"}
          onOpen={() => actions.open({ page: "dropdown-menu" })}
          testId={"open-dropdown-menu"}
        >
          <DropdownMenuSpecimen />
        </Tile>
      </Category>
      <Category title={"Content"}>
        <Tile
          name={"Card"}
          tagline={"Groups related content"}
          onOpen={() => actions.open({ page: "card" })}
          testId={"open-card"}
        >
          <CardSpecimen />
        </Tile>
        <Tile
          name={"Item"}
          tagline={"One row of a list"}
          onOpen={() => actions.open({ page: "item" })}
          testId={"open-item"}
        >
          <ItemSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
