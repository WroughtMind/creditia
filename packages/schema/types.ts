// credits.json v0 的 TypeScript 类型。与 credits.schema.json 同步维护。
export type Evidence = 'verified' | 'declared' | 'unattributed';
export type DateStr = `${number}-${number}-${number}`;

export interface Credits {
  $schema?: string;
  schemaVersion: 0;
  project: { id?: string; name: string; repos?: string[]; range?: { from?: DateStr; to?: DateStr } };
  generatedAt: string;
  branch?: string;
  measurement?: { metric: 'added-lines'; scope: 'git-history'; timezone: 'UTC'; humanValueEstimated: false; aiInference: false };
  sources?: string[];
  head?: string;
  versions?: { tag: string; date: DateStr }[];
  coverage?: { name?: string; from: DateStr; to: DateStr; best: Evidence; verifiedShare?: number }[];
  people?: { name: string; roles: string[]; note?: string }[];
  contributors: Contributor[];
  totals: Partial<Totals>;
  breakdown: Breakdown[];
  human?: Partial<{ prompts: number; reviewRecords: number; tabAssistedLines: number; agentReviews: number }>;
  timeline?: { week: string; byContributor: Record<string, number> }[];
  days?: { date: DateStr; by: Record<string, number> }[];
  clock?: number[][];
  files?: { path?: string; lines: number; by: Record<string, number> }[];
  sessions?: { contributor: string; tool?: string; model?: string; provider?: string | null; start: number; end: number; tokens?: number; review?: boolean }[];
  units: Unit[];
  conflictLines?: number;
}

export interface Contributor {
  id: string;
  kind: 'agent' | 'human' | 'unattributed';
  harness?: string;
  model?: string | null;
  family?: string | null;
  provider?: string | null;
  where?: 'local' | 'cloud' | 'unknown';
  name?: string;
  roles: string[];
  reviews?: number;
}

export interface Totals {
  commits: number;
  units: number;
  linesAdded: number;
  linesSurviving: number;
  linesWritten: number;
  tokens: number;
  sessions: number;
  days: number;
  binaryFiles: number;
}

export interface Breakdown {
  contributor: string;
  linesAdded: number;
  linesSurviving?: number;
  byEvidence?: Record<string, { linesAdded?: number; units?: number }>;
  byArea?: Record<string, number>;
  byVersion?: Record<string, number>;
}

export interface Unit {
  id: string;
  title: string;
  date: DateStr;
  version?: string | null;
  commits: string[];
  areas?: string[];
  lines?: number;
  shares: { contributor: string; share: number; evidence: Evidence }[];
}
