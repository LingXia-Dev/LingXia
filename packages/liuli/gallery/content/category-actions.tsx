// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as ButtonSpecimen } from '../../core/src/Button/Button.examples';
import { Specimen as ToggleButtonSpecimen } from '../../core/src/ToggleButton/ToggleButton.examples';
import { Specimen as IconButtonSpecimen } from '../../core/src/IconButton/IconButton.examples';
import { Specimen as ButtonGroupSpecimen } from '../../core/src/ButtonGroup/ButtonGroup.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Buttons that start something, or turn a setting on and off."}</text>
      <Category title={"4 components"}>
        <Tile
          name={"Button"}
          tagline={"Starts an action"}
          onOpen={() => onOpen("button")}
          testId={"open-button"}
        >
          <ButtonSpecimen />
        </Tile>
        <Tile
          name={"ToggleButton"}
          tagline={"Stays down while on"}
          onOpen={() => onOpen("toggle-button")}
          testId={"open-toggle-button"}
        >
          <ToggleButtonSpecimen />
        </Tile>
        <Tile
          name={"IconButton"}
          tagline={"An icon that acts"}
          onOpen={() => onOpen("icon-button")}
          testId={"open-icon-button"}
        >
          <IconButtonSpecimen />
        </Tile>
        <Tile
          name={"ButtonGroup"}
          tagline={"Buttons together"}
          onOpen={() => onOpen("button-group")}
          testId={"open-button-group"}
        >
          <ButtonGroupSpecimen />
        </Tile>
      </Category>
    </>
  );
}
