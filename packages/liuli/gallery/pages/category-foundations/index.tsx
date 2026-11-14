// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { ColourSpecimen, TypographySpecimen, ShapeAndSpacingSpecimen, ElevationSpecimen, MotionSpecimen } from '../../shared/Specimens';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"The colour roles, type, shape, elevation and motion every component draws from."}</text>
      <Category title={"5 guides"}>
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
    </GalleryPage>
  );
}
