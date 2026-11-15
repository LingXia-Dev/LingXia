// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as TabsSpecimen } from '../../core/src/Tabs/Tabs.examples';
import { Specimen as NavListSpecimen } from '../../core/src/NavList/NavList.examples';
import { Specimen as BreadcrumbSpecimen } from '../../core/src/Breadcrumb/Breadcrumb.examples';
import { Specimen as PaginationSpecimen } from '../../core/src/Pagination/Pagination.examples';
import { Specimen as StepperSpecimen } from '../../core/src/Stepper/Stepper.examples';
import { Specimen as LinkSpecimen } from '../../core/src/Link/Link.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Ways to move between views, through a set, and through the steps of a task."}</text>
      <Category title={"6 components"}>
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
          onOpen={() => onOpen("breadcrumb")}
          testId={"open-breadcrumb"}
        >
          <BreadcrumbSpecimen />
        </Tile>
        <Tile
          name={"Pagination"}
          tagline={"A page at a time"}
          onOpen={() => onOpen("pagination")}
          testId={"open-pagination"}
        >
          <PaginationSpecimen />
        </Tile>
        <Tile
          name={"Stepper"}
          tagline={"Steps of a task"}
          onOpen={() => onOpen("stepper")}
          testId={"open-stepper"}
        >
          <StepperSpecimen />
        </Tile>
        <Tile
          name={"Link"}
          tagline={"Words that lead on"}
          onOpen={() => onOpen("link")}
          testId={"open-link"}
        >
          <LinkSpecimen />
        </Tile>
      </Category>
    </>
  );
}
