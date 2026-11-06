export interface LabPlots { hourly: number[]; share: number[]; latency: number[] }
export interface LabClause { heading: string; term: string; body: string }
export interface LabPageData { plots: LabPlots; terms: { updated: string; clauses: LabClause[] } }
