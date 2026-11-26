import type { Kit, Tile } from './kits.ts';

const catalogs = new WeakMap<Kit, Tile[]>();
export const tileKey = (tile: Tile) => `${tile.id}#${tile.variant ?? ''}`;

/** 先保留展墙原来的作品顺序，再补齐版式；默认版式和显式列出的同一版式只出现一次。 */
export function wallTiles(kit: Kit): Tile[] {
  const cached = catalogs.get(kit);
  if (cached) return cached;
  const out: Tile[] = [], seen = new Set<string>();
  const add = (tile: Tile, variant = kit.current(tile.id, tile.variant)) => {
    const item = { ...tile, variant }, key = tileKey(item);
    if (!seen.has(key)) { seen.add(key); out.push(item); }
  };
  kit.board.forEach((tile) => add(tile));
  for (const tile of kit.board) for (const variant of kit.variants(tile.id)) add(tile, variant);
  catalogs.set(kit, out);
  return out;
}

export const wallVariant = (kit: Kit, selected: Record<string, string>, key: string) => {
  const [id, variant] = key.split('#');
  return kit.current(id, selected[key] ?? (variant || undefined));
};

/** 换到已有版式时交换两个位置，整面墙仍然完整且不重复。返回发生变化的位置。 */
export function selectWallVariant(kit: Kit, selected: Record<string, string>, key: string, variant: string): string[] {
  const list = wallTiles(kit), tile = list.find((item) => tileKey(item) === key);
  if (!tile || !list.some((item) => item.id === tile.id && item.variant === variant)) return [];
  const before = wallVariant(kit, selected, key);
  if (before === variant) return [];
  const peer = list.find((item) => item.id === tile.id && wallVariant(kit, selected, tileKey(item)) === variant);
  const changed = [key];
  if (peer) { const other = tileKey(peer); selected[other] = before ?? ''; changed.push(other); }
  selected[key] = variant;
  return changed;
}
