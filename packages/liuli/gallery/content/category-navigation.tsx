// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as TabsSpecimen } from '../../core/src/Tabs/Tabs.examples';
import { Specimen as NavListSpecimen } from '../../core/src/NavList/NavList.examples';
import { Specimen as BreadcrumbSpecimen } from '../../core/src/Breadcrumb/Breadcrumb.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Ways to move between views on the same page."}</text>
      <Category title={"3 components"}>
        <Tile
          name={"Tabs"}
          tagline={"A few peer views"}
          onOpen={() => onOpen("tabs")}
          testId={"open-tabs"}
        >
          <TabsSpecimen />
        </Tile>
        <Tile
          name={"NavList"}
          tagline={"A sidebar of places"}
          onOpen={() => onOpen("nav-list")}
          testId={"open-nav-list"}
        >
          <NavListSpecimen />
        </Tile>
        <Tile
          name={"Breadcrumb"}
          tagline={"The way back up"}
          isWide
          onOpen={() => onOpen("breadcrumb")}
          testId={"open-breadcrumb"}
        >
          <BreadcrumbSpecimen />
        </Tile>
      </Category>
    </>
  );
}
