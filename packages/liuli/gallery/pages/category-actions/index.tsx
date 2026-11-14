// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as ButtonSpecimen } from '../../../core/src/Button/Button.examples';
import { Specimen as ToggleButtonSpecimen } from '../../../core/src/ToggleButton/ToggleButton.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Buttons that start something, or turn a setting on and off."}</text>
      <Category title={"2 components"}>
        <Tile
          name={"Button"}
          tagline={"Starts an action"}
          onOpen={() => actions.open({ page: "button" })}
          testId={"open-button"}
        >
          <ButtonSpecimen />
        </Tile>
        <Tile
          name={"ToggleButton"}
          tagline={"Stays down while on"}
          onOpen={() => actions.open({ page: "toggle-button" })}
          testId={"open-toggle-button"}
        >
          <ToggleButtonSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
