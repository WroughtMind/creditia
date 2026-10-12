// 全部风格。顺序就是 App 左下角圆点的顺序：基础 → 东方 → 西方古典 → 电影与文学 → 流行
import type { Style } from '../style.ts';
import { minimal, noir, glass } from './base.ts';
import { ink, ukiyoe, woodblock } from './east.ts';
import { cinema, baroque, grimoire } from './west.ts';
import { manuscript, meadow, pastel, cutout } from './story.ts';
import { manga, terminal } from './pop.ts';
import { cyber } from './cyber.ts';

export const STYLES: Style[] = [minimal, noir, glass, ink, ukiyoe, woodblock, cinema, baroque, grimoire, manuscript, pastel, cutout, meadow, manga, cyber, terminal];
export const styleById = (id: string) => STYLES.find((s) => s.id === id);
