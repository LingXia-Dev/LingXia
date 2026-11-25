// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as JinshuViewSpecimen } from '../../jinshu/src/JinshuView/JinshuView.examples';
import { Specimen as JinshuEditorSpecimen } from '../../jinshu/src/JinshuEditor/JinshuEditor.examples';
import { Specimen as JinshuDiffSpecimen } from '../../jinshu/src/JinshuDiff/JinshuDiff.examples';
import { Specimen as JinshuOutlineSpecimen } from '../../jinshu/src/JinshuOutline/JinshuOutline.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Structured documents that people and the app's agent read, edit and compare: a lesson, an article, a handbook."}</text>
      <Category title={"4 components"}>
        <Tile
          name={"JinshuView"}
          tagline={"A document, read"}
          onOpen={() => onOpen("jinshu-view")}
          testId={"open-jinshu-view"}
        >
          <JinshuViewSpecimen />
        </Tile>
        <Tile
          name={"JinshuEditor"}
          tagline={"A document, edited"}
          onOpen={() => onOpen("jinshu-editor")}
          testId={"open-jinshu-editor"}
        >
          <JinshuEditorSpecimen />
        </Tile>
        <Tile
          name={"JinshuDiff"}
          tagline={"What changed"}
          onOpen={() => onOpen("jinshu-diff")}
          testId={"open-jinshu-diff"}
        >
          <JinshuDiffSpecimen />
        </Tile>
        <Tile
          name={"JinshuOutline"}
          tagline={"Contents, progress"}
          onOpen={() => onOpen("jinshu-outline")}
          testId={"open-jinshu-outline"}
        >
          <JinshuOutlineSpecimen />
        </Tile>
      </Category>
    </>
  );
}
