// Canonical desktop wall order, including repeated badge layouts and wide spans.
export interface BoardTile { id: string; variant?: string; span?: number }
export const CLASSIC_BOARD: BoardTile[] = [
  { id: 'share', span: 2 }, { id: 'tier' }, { id: 'people' }, { id: 'roll' }, { id: 'receipt' }, { id: 'donut' },
  { id: 'asterisk' }, { id: 'ribbon', span: 2 }, { id: 'waffle' }, { id: 'mosaic' }, { id: 'versions' }, { id: 'area' },
  { id: 'stream', span: 2 }, { id: 'survival' }, { id: 'evidence' }, { id: 'areas' },
  { id: 'lanes' }, { id: 'clock' }, { id: 'settle' }, { id: 'app' }, { id: 'fetch' },
  { id: 'codemap', span: 2 }, { id: 'calendar', span: 2 },
  { id: 'byline', span: 2 }, { id: 'badge' }, { id: 'badge', variant: 'emblem' }, { id: 'badge', variant: 'stack' },
];
export const V2_BOARD: BoardTile[] = [
  { id: 'share', span: 2 }, { id: 'tier' }, { id: 'people' }, { id: 'roll' }, { id: 'receipt' }, { id: 'donut' }, { id: 'asterisk' },
  { id: 'stream', span: 2 }, { id: 'waffle' }, { id: 'survival' }, { id: 'evidence' }, { id: 'versions' }, { id: 'areas' }, { id: 'lanes', span: 2 }, { id: 'clock', span: 2 },
  { id: 'mosaic' }, { id: 'codemap', span: 2 }, { id: 'calendar', span: 2 }, { id: 'badge', variant: 'emblem' }, { id: 'badge', variant: 'stack' }, { id: 'badge', variant: 'flat' }, { id: 'byline', span: 2 },
];
