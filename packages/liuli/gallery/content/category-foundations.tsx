// Generated from the doc file by scripts/generate.mjs; do not edit.
import { ColourSpecimen, TypographySpecimen, ShapeAndSpacingSpecimen, ElevationSpecimen, MotionSpecimen } from '../shared/Specimens';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"The colour roles, type, shape, elevation and motion every component draws from."}</text>
      <Category title={"5 guides"}>
        <Tile
          name={"Colour"}
          tagline={"Theme colour roles"}
          onOpen={() => onOpen("colour")}
          testId={"open-colour"}
        >
          <ColourSpecimen />
        </Tile>
        <Tile
          name={"Typography"}
          tagline={"Sizes and weights"}
          onOpen={() => onOpen("typography")}
          testId={"open-typography"}
        >
          <TypographySpecimen />
        </Tile>
        <Tile
          name={"Shape & spacing"}
          tagline={"Corners and heights"}
          onOpen={() => onOpen("shape-and-spacing")}
          testId={"open-shape-and-spacing"}
        >
          <ShapeAndSpacingSpecimen />
        </Tile>
        <Tile
          name={"Elevation"}
          tagline={"Raised and modal"}
          onOpen={() => onOpen("elevation")}
          testId={"open-elevation"}
        >
          <ElevationSpecimen />
        </Tile>
        <Tile
          name={"Motion"}
          tagline={"Enter, move, react"}
          isWide
          onOpen={() => onOpen("motion")}
          testId={"open-motion"}
        >
          <MotionSpecimen />
        </Tile>
      </Category>
    </>
  );
}
