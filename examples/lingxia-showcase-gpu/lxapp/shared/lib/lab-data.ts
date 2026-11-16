import type { LabPageData } from '../lab-types';

const body = 'This section says what the service does for you and what it asks of you in return. It applies from the day you create an account until the day you close it.';

/** The series and the document the catalogue's plot and long-document demos draw. */
export function labData(): LabPageData {
  return {
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
}
