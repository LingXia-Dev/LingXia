// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as StatSpecimen } from '../../../charts/src/Stat/Stat.examples';
import { Specimen as SparklineSpecimen } from '../../../charts/src/Sparkline/Sparkline.examples';
import { Specimen as BarChartSpecimen } from '../../../charts/src/BarChart/BarChart.examples';
import { Specimen as LineChartSpecimen } from '../../../charts/src/LineChart/LineChart.examples';
import { Specimen as PieChartSpecimen } from '../../../charts/src/PieChart/PieChart.examples';
import { Specimen as ChartSpecimen } from '../../../charts/src/Chart/Chart.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Figures and small charts that show numbers at a glance."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"Stat"}
          tagline={"A number to watch"}
          onOpen={() => actions.open({ page: "stat" })}
          testId={"open-stat"}
        >
          <StatSpecimen />
        </Tile>
        <Tile
          name={"Sparkline"}
          tagline={"A trend at a glance"}
          onOpen={() => actions.open({ page: "sparkline" })}
          testId={"open-sparkline"}
        >
          <SparklineSpecimen />
        </Tile>
        <Tile
          name={"BarChart"}
          tagline={"Compare amounts"}
          onOpen={() => actions.open({ page: "bar-chart" })}
          testId={"open-bar-chart"}
        >
          <BarChartSpecimen />
        </Tile>
        <Tile
          name={"LineChart"}
          tagline={"A number over time"}
          onOpen={() => actions.open({ page: "line-chart" })}
          testId={"open-line-chart"}
        >
          <LineChartSpecimen />
        </Tile>
        <Tile
          name={"PieChart"}
          tagline={"Parts of a whole"}
          onOpen={() => actions.open({ page: "pie-chart" })}
          testId={"open-pie-chart"}
        >
          <PieChartSpecimen />
        </Tile>
        <Tile
          name={"Chart"}
          tagline={"A chart's frame"}
          onOpen={() => actions.open({ page: "chart" })}
          testId={"open-chart"}
        >
          <ChartSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
