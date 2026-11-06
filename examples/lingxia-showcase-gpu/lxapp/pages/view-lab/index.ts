import type { LabPageData } from '../../shared/lab-types';

const body = 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.';
const data: LabPageData = {
  plots: {
    hourly: Array.from({ length: 24 }, (_, hour) => Math.round(38 + 18 * Math.sin((hour - 6) / 3.8))),
    share: [45, 25, 15, 10, 5],
    latency: Array.from({ length: 30 }, (_, index) => 10 + (index * 7) % 9),
  },
  terms: {
    updated: '2026-10-01',
    clauses: ['Service', 'Account', 'Content', 'Privacy', 'Termination'].map((term, index) => ({ heading: `Clause ${index + 1}`, term, body })),
  },
};

Page({ data });
