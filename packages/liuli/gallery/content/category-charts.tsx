// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as StatSpecimen } from '../../charts/src/Stat/Stat.examples';
import { Specimen as SparklineSpecimen } from '../../charts/src/Sparkline/Sparkline.examples';
import { Specimen as BarChartSpecimen } from '../../charts/src/BarChart/BarChart.examples';
import { Specimen as LineChartSpecimen } from '../../charts/src/LineChart/LineChart.examples';
import { Specimen as PieChartSpecimen } from '../../charts/src/PieChart/PieChart.examples';
import { Specimen as ChartSpecimen } from '../../charts/src/Chart/Chart.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Figures and small charts that show numbers at a glance."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"Stat"}
          tagline={"A number to watch"}
          onOpen={() => onOpen("stat")}
          testId={"open-stat"}
        >
          <StatSpecimen />
        </Tile>
        <Tile
          name={"Sparkline"}
          tagline={"A trend at a glance"}
          onOpen={() => onOpen("sparkline")}
          testId={"open-sparkline"}
        >
          <SparklineSpecimen />
        </Tile>
        <Tile
          name={"BarChart"}
          tagline={"Compare amounts"}
          onOpen={() => onOpen("bar-chart")}
          testId={"open-bar-chart"}
        >
          <BarChartSpecimen />
        </Tile>
        <Tile
          name={"LineChart"}
          tagline={"A number over time"}
          onOpen={() => onOpen("line-chart")}
          testId={"open-line-chart"}
        >
          <LineChartSpecimen />
        </Tile>
        <Tile
          name={"PieChart"}
          tagline={"Parts of a whole"}
          onOpen={() => onOpen("pie-chart")}
          testId={"open-pie-chart"}
        >
          <PieChartSpecimen />
        </Tile>
        <Tile
          name={"Chart"}
          tagline={"A chart's frame"}
          onOpen={() => onOpen("chart")}
          testId={"open-chart"}
        >
          <ChartSpecimen />
        </Tile>
      </Category>
    </>
  );
}
