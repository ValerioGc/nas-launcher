import type { CSSProperties } from 'vue';
import type { DesktopBackground } from './types/appearance';
import { inkShadow, over, readableInk } from '@/utils/contrast';

/**
 * Local images allowed, the same ones accepted by the backend. The label is an i18n key: the
 * files are listed once, and the settings view reads the list instead of repeating it.
 */
export const WALLPAPERS = [
  { file: '/wallpapers/landscape.svg', label: 'landscape' },
  { file: '/wallpapers/waves.svg', label: 'waves' },
  { file: '/wallpapers/aurora.svg', label: 'aurora' },
  { file: '/wallpapers/shapes.svg', label: 'shapes' },
  { file: '/wallpapers/topography.svg', label: 'topography' },
  { file: '/wallpapers/mesh.svg', label: 'mesh' },
] as const;

const IMAGES: string[] = WALLPAPERS.map((wallpaper) => wallpaper.file);

/**
 * The pattern drawn in `tint`, with the size of its tile when it repeats. `size` is in pixels for
 * the tiles and in percent for the ellipses, `angle` in degrees.
 */
function patternLayer(
  pattern: DesktopBackground['pattern'],
  tint: string,
  size: number,
  angle: number,
): { image: string; size?: string } {
  switch (pattern) {
    case 'linear':
      return { image: `linear-gradient(${angle}deg, ${tint}, transparent)` };
    case 'dots':
      return {
        image: `radial-gradient(circle, ${tint} 18%, transparent 20%)`,
        size: `${size}px ${size}px`,
      };
    case 'rings':
      return {
        image: `repeating-radial-gradient(circle at 50% 50%, ${tint} 0 2px, transparent 2px ${size}px)`,
      };
    case 'grid':
      return {
        image:
          `repeating-linear-gradient(0deg, ${tint} 0 1px, transparent 1px ${size}px), ` +
          `repeating-linear-gradient(90deg, ${tint} 0 1px, transparent 1px ${size}px)`,
      };
    case 'stripes':
      return {
        image: `repeating-linear-gradient(${angle}deg, ${tint} 0 ${Math.round(size / 4)}px, transparent ${Math.round(size / 4)}px ${size}px)`,
      };
    default: {
      const x = 50 + Math.cos((angle * Math.PI) / 180) * 30;
      const y = 50 + Math.sin((angle * Math.PI) / 180) * 30;
      const ellipse = `radial-gradient(ellipse ${size}% ${size}% at ${x}% ${y}%, ${tint}, transparent)`;
      return pattern === 'multiple'
        ? {
            image: `${ellipse}, radial-gradient(ellipse ${size}% ${size}% at ${100 - x}% ${100 - y}%, ${tint}, transparent)`,
          }
        : { image: ellipse };
    }
  }
}

/**
 * Values come from bounded settings. Image paths are allowlisted on both sides.
 **/
export function backgroundStyle(value: DesktopBackground): CSSProperties {
  const color = /^#[\da-f]{6}$/i.test(value.color) ? value.color : '#258ac0';
  const opacity = Math.min(100, Math.max(0, value.intensity)) / 100;
  const size = Math.min(100, Math.max(10, value.size));
  const angle = Math.min(360, Math.max(0, value.direction));
  const alpha = Math.round(opacity * 255)
    .toString(16)
    .padStart(2, '0');

  const tint = `${color}${alpha}`;
  let backgroundImage: string;
  let backgroundSize = '100% 100%';

  //  Background mode.
  if (value.mode === 'image') {
    const image = IMAGES.includes(value.image) ? value.image : IMAGES[0];
    backgroundImage = `url("${image}")`;
    backgroundSize = 'cover';
  } else {
    const layer = patternLayer(value.pattern, tint, size, angle);
    backgroundImage = layer.image;
    backgroundSize = layer.size ?? backgroundSize;
  }

  return {
    backgroundImage,
    backgroundSize,
    backgroundPosition: 'center',
    filter: `blur(${Math.min(30, Math.max(0, value.blur))}px)`,
    opacity: value.mode === 'image' ? opacity : 1,
  };
}

/**
 * The ink the desktop labels are written in, and the halo behind them.
 *
 * @param value - The background as it is drawn.
 * @param surface - The theme colour underneath it, as `#rrggbb`.
 */
export function desktopInk(value: DesktopBackground, surface: string) {
  const colour = /^#[\da-f]{6}$/i.test(value.color) ? value.color : '#258ac0';
  const effective =
    value.mode === 'image'
      ? surface
      : over(colour, Math.min(100, Math.max(0, value.intensity)) / 100, surface);
  const ink = readableInk(effective);

  return { ink, shadow: inkShadow(ink) };
}
