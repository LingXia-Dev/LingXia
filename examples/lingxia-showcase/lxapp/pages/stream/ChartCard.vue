<template>
  <div class="mt-3 overflow-hidden rounded-2xl border border-line-200 bg-surface-50 shadow-sm chart-in">
    <p class="px-3.5 pb-0.5 pt-3 text-[10px] font-semibold uppercase tracking-widest text-gray-400">
      {{ data.title }}
    </p>
    <div ref="containerRef" :style="{ width: '100%', height: `${height}px` }" />
  </div>
</template>

<script setup lang="ts">
import * as echarts from 'echarts';
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { ChartData } from './index';
import {
  chartAxisColors,
  getResolvedTheme,
  subscribeTheme,
  type ChartAxisColors,
  type ResolvedTheme,
} from '../../shared/lib/theme';

const props = defineProps<{
  data: ChartData;
}>();

const PALETTE = ['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444', '#06b6d4'];

// ECharts paints into SVG it owns, so the theme is passed in as resolved values.
function buildOption(data: ChartData, axis: ChartAxisColors): echarts.EChartsOption {
  const labels = data.series.map((item) => item.label);
  const values = data.series.map((item) => item.value);

  if (data.kind === 'pie') {
    return {
      color: PALETTE,
      tooltip: { trigger: 'item', formatter: '{b}: {d}%' },
      legend: {
        bottom: 0,
        textStyle: { fontSize: 11, color: axis.label },
        icon: 'circle',
        itemWidth: 8,
        itemHeight: 8,
        itemGap: 12,
      },
      series: [
        {
          type: 'pie',
          radius: ['40%', '68%'],
          center: ['50%', '44%'],
          data: data.series.map((item) => ({ name: item.label, value: item.value })),
          label: { show: false },
          emphasis: {
            label: { show: true, fontSize: 13, fontWeight: 'bold' },
            scale: true,
            scaleSize: 5,
          },
          animationType: 'scale',
          animationEasing: 'elasticOut',
        },
      ],
    };
  }

  const isLine = data.kind === 'line';
  return {
    color: PALETTE,
    grid: { top: 10, right: 10, bottom: 24, left: 10, containLabel: true },
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'category',
      data: labels,
      axisLine: { lineStyle: { color: axis.line } },
      axisTick: { show: false },
      axisLabel: { fontSize: 11, color: axis.label },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: axis.split, type: 'dashed' } },
      axisLabel: { fontSize: 11, color: axis.label },
      axisLine: { show: false },
      axisTick: { show: false },
    },
    series: [
      {
        type: isLine ? 'line' : 'bar',
        data: values,
        smooth: isLine ? 0.4 : false,
        symbolSize: isLine ? 6 : undefined,
        areaStyle: isLine
          ? {
              color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
                { offset: 0, color: 'rgba(59,130,246,0.22)' },
                { offset: 1, color: 'rgba(59,130,246,0)' },
              ]),
            }
          : undefined,
        lineStyle: isLine ? { width: 2.5 } : undefined,
        itemStyle: { borderRadius: isLine ? undefined : [4, 4, 0, 0] },
        barMaxWidth: 36,
      },
    ],
  };
}

const containerRef = ref<HTMLDivElement | null>(null);
const height = computed(() => (props.data.kind === 'pie' ? 210 : 180));
const theme = ref<ResolvedTheme>(getResolvedTheme());
let chart: echarts.ECharts | null = null;
let unsubscribeTheme: (() => void) | null = null;

function renderChart() {
  if (!containerRef.value) return;
  if (!chart) {
    chart = echarts.init(containerRef.value, null, { renderer: 'svg' });
  }
  chart.setOption(buildOption(props.data, chartAxisColors()));
}

onMounted(() => {
  renderChart();
  unsubscribeTheme = subscribeTheme((next) => {
    theme.value = next;
  });
});

watch(() => props.data, renderChart, { deep: true });
watch(theme, renderChart);

onBeforeUnmount(() => {
  unsubscribeTheme?.();
  unsubscribeTheme = null;
  chart?.dispose();
  chart = null;
});
</script>

<style scoped>
.chart-in {
  animation: chart-in 220ms ease-out;
}

@keyframes chart-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
