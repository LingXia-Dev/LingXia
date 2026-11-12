// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as AvatarSpecimen } from '../../../core/src/Avatar/Avatar.examples';
import { Specimen as BadgeSpecimen } from '../../../core/src/Badge/Badge.examples';
import { Specimen as BannerSpecimen } from '../../../core/src/Banner/Banner.examples';
import { Specimen as ButtonSpecimen } from '../../../core/src/Button/Button.examples';
import { Specimen as CardSpecimen } from '../../../core/src/Card/Card.examples';
import { Specimen as CheckboxInputSpecimen } from '../../../core/src/CheckboxInput/CheckboxInput.examples';
import { Specimen as DetailsSpecimen } from '../../../core/src/Details/Details.examples';
import { Specimen as DialogSpecimen } from '../../../core/src/Dialog/Dialog.examples';
import { Specimen as DropdownMenuSpecimen } from '../../../core/src/DropdownMenu/DropdownMenu.examples';
import { Specimen as FieldSpecimen } from '../../../core/src/Field/Field.examples';
import { Specimen as ItemSpecimen } from '../../../core/src/Item/Item.examples';
import { Specimen as ProgressBarSpecimen } from '../../../core/src/ProgressBar/ProgressBar.examples';
import { Specimen as RadioGroupSpecimen } from '../../../core/src/RadioGroup/RadioGroup.examples';
import { Specimen as SelectSpecimen } from '../../../core/src/Select/Select.examples';
import { Specimen as SliderSpecimen } from '../../../core/src/Slider/Slider.examples';
import { Specimen as SwitchSpecimen } from '../../../core/src/Switch/Switch.examples';
import { Specimen as TabsSpecimen } from '../../../core/src/Tabs/Tabs.examples';
import { Specimen as TextInputSpecimen } from '../../../core/src/TextInput/TextInput.examples';
import { Specimen as TooltipSpecimen } from '../../../core/src/Tooltip/Tooltip.examples';
import { Category, GalleryPage, Masthead, Tile } from '../../shared/Layout';
import { ColourSpecimen, TypographySpecimen, ShapeAndSpacingSpecimen, ElevationSpecimen, MotionSpecimen, SignInSpecimen, SettingsSpecimen, UploadsSpecimen } from '../../shared/Specimens';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <Masthead version={"0.1.0"} status={"Preview"} />
      <Category title={"Foundations"}>
        <Tile
          name={"Colour"}
          tagline={"Theme colour roles"}
          onOpen={() => actions.open({ page: "colour" })}
          testId={"open-colour"}
        >
          <ColourSpecimen />
        </Tile>
        <Tile
          name={"Typography"}
          tagline={"Sizes and weights"}
          onOpen={() => actions.open({ page: "typography" })}
          testId={"open-typography"}
        >
          <TypographySpecimen />
        </Tile>
        <Tile
          name={"Shape & spacing"}
          tagline={"Corners and heights"}
          onOpen={() => actions.open({ page: "shape-and-spacing" })}
          testId={"open-shape-and-spacing"}
        >
          <ShapeAndSpacingSpecimen />
        </Tile>
        <Tile
          name={"Elevation"}
          tagline={"Raised and modal"}
          onOpen={() => actions.open({ page: "elevation" })}
          testId={"open-elevation"}
        >
          <ElevationSpecimen />
        </Tile>
        <Tile
          name={"Motion"}
          tagline={"Enter, move, react"}
          isWide
          onOpen={() => actions.open({ page: "motion" })}
          testId={"open-motion"}
        >
          <MotionSpecimen />
        </Tile>
      </Category>
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
      <Category title={"Navigation"}>
        <Tile
          name={"Tabs"}
          tagline={"A few peer views"}
          isWide
          onOpen={() => actions.open({ page: "tabs" })}
          testId={"open-tabs"}
        >
          <TabsSpecimen />
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
          name={"Select"}
          tagline={"One of many"}
          onOpen={() => actions.open({ page: "select" })}
          testId={"open-select"}
        >
          <SelectSpecimen />
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
          tagline={"A status label"}
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
        <Tile
          name={"Banner"}
          tagline={"Stays until fixed"}
          isWide
          onOpen={() => actions.open({ page: "banner" })}
          testId={"open-banner"}
        >
          <BannerSpecimen />
        </Tile>
      </Category>
      <Category title={"Overlays"}>
        <Tile
          name={"Dialog"}
          tagline={"A decision, on top"}
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
        <Tile
          name={"Tooltip"}
          tagline={"A note on demand"}
          isWide
          onOpen={() => actions.open({ page: "tooltip" })}
          testId={"open-tooltip"}
        >
          <TooltipSpecimen />
        </Tile>
      </Category>
      <Category title={"Content"}>
        <Tile
          name={"Card"}
          tagline={"Related content"}
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
        <Tile
          name={"Details"}
          tagline={"More, behind a row"}
          onOpen={() => actions.open({ page: "details" })}
          testId={"open-details"}
        >
          <DetailsSpecimen />
        </Tile>
        <Tile
          name={"Avatar"}
          tagline={"Who, at a glance"}
          onOpen={() => actions.open({ page: "avatar" })}
          testId={"open-avatar"}
        >
          <AvatarSpecimen />
        </Tile>
      </Category>
      <Category title={"Patterns"}>
        <Tile
          name={"Sign in"}
          tagline={"A form that checks"}
          onOpen={() => actions.open({ page: "sign-in" })}
          testId={"open-sign-in"}
        >
          <SignInSpecimen />
        </Tile>
        <Tile
          name={"Settings"}
          tagline={"Rows and a confirm"}
          onOpen={() => actions.open({ page: "settings" })}
          testId={"open-settings"}
        >
          <SettingsSpecimen />
        </Tile>
        <Tile
          name={"Uploads"}
          tagline={"Long-running work"}
          isWide
          onOpen={() => actions.open({ page: "uploads" })}
          testId={"open-uploads"}
        >
          <UploadsSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
