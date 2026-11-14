// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as ButtonSpecimen } from '../../core/src/Button/Button.examples';
import { Specimen as ToggleButtonSpecimen } from '../../core/src/ToggleButton/ToggleButton.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Buttons that start something, or turn a setting on and off."}</text>
      <Category title={"2 components"}>
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
      </Category>
    </>
  );
}
