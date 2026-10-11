// credits.json v0 的 TypeScript 类型。与 credits.schema.json 同步维护。
export type Evidence = 'verified' | 'manual' | 'declared' | 'inferred' | 'unattributed';
export type DateStr = `${number}-${number}-${number}`;

export interface Credits {
  $schema?: string;
  schemaVersion: 0;
  project: { name: string; repos?: string[]; range?: { from?: DateStr; to?: DateStr } };
  generatedAt: string;
  level?: 'strict' | 'standard' | 'loose';
  head?: string;
  versions?: { tag: string; date: DateStr }[];
  coverage?: { name?: string; from: DateStr; to: DateStr; best: Evidence; verifiedShare?: number }[];
  contributors: Contributor[];
  totals: Partial<Totals>;
  breakdown: Breakdown[];
  human?: Partial<{ prompts: number; plans: number; reviews: number; tabAssistedLines: number; agentReviews: number }>;
  timeline?: { week: string; byContributor: Record<string, number> }[];
  days?: { date: DateStr; by: Record<string, number> }[];
  clock?: number[][];
  files?: { path?: string; lines: number; by: Record<string, number> }[];
  sessions?: { contributor: string; start: number; end: number; tokens?: number; review?: boolean; prompts?: number }[];
  units: Unit[];
  conflictLines?: number;
}

export interface Contributor {
  id: string;
  kind: 'agent' | 'human' | 'unattributed';
  harness?: string;
  model?: string | null;
  family?: string | null;
  where?: 'local' | 'cloud';
  name?: string;
  roles: ('author' | 'reviewer' | 'planner')[];
  /** 谁提供的模型：anthropic、aws-bedrock、google-vertex、openrouter、relay、自定义渠道名 */
  provider?: string;
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
