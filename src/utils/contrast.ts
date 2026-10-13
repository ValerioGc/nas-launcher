/**
 * Whether a label can be read over what is behind it.
 *
 * The desktop lets a colour, a pattern and an image be chosen freely, which means a label can end
 * up white on white. Rather than picking a text colour by hand for each wallpaper, the effective
 * background is worked out from the same values the background itself is drawn with, and the ink
 * that reads better over it wins.
 *
 * The numbers are the ones from WCAG: relative luminance with the sRGB transfer curve, and a
 * contrast ratio of `(lighter + 0.05) / (darker + 0.05)`.
 */

// Base inks for the desktop: near-black and near-white.
export const DARK_INK = '#12191f';
export const LIGHT_INK = '#f6faf8';

/**
 * Converts a `#rrggbb` colour string to its three RGB channels.
 * @returns An array of three numbers representing the RGB channels, or black ([0, 0, 0]) for anything that is not a valid colour.
 */
export function channels(hex: string): [number, number, number] {
  const digits = /^#([\da-f]{6})$/i.exec(hex.trim())?.[1];
  if (!digits) {
    return [0, 0, 0];
  }

  const value = Number.parseInt(digits, 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

// Relative luminance (0 = black, 1 = white).
export function luminance(hex: string): number {
  const [red, green, blue] = channels(hex).map((channel) => {
    const ratio = channel / 255;

    return ratio <= 0.04045 ? ratio / 12.92 : ((ratio + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];

  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

// Contrast ratio (lighter + 0.05) / (darker + 0.05)
export function contrast(one: string, other: string): number {
  const first = luminance(one);
  const second = luminance(other);
  const lighter = Math.max(first, second);
  const darker = Math.min(first, second);

  return (lighter + 0.05) / (darker + 0.05);
}

// `top` laid over `bottom` at `alpha`, which is what a tinted background actually shows.
export function over(top: string, alpha: number, bottom: string): string {
  const weight = Math.min(1, Math.max(0, alpha));
  const front = channels(top);
  const back = channels(bottom);

  return `#${front
    .map((channel, index) =>
      Math.round(channel * weight + back[index]! * (1 - weight))
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`;
}

export function readableInk(background: string): string {
  return contrast(LIGHT_INK, background) >= contrast(DARK_INK, background) ? LIGHT_INK : DARK_INK;
}

export function inkShadow(ink: string): string {
  const halo = ink === LIGHT_INK ? DARK_INK : LIGHT_INK;
  return `0 1px 2px ${halo}, 0 0 6px ${halo}`;
}
