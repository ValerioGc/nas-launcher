import { describe, expect, it } from 'vitest';
import {
  channels,
  contrast,
  DARK_INK,
  inkShadow,
  LIGHT_INK,
  luminance,
  over,
  readableInk,
} from '@/utils/contrast';

describe('label readability', () => {
  it('reads the three channels of a colour and ignores anything else', () => {
    expect(channels('#ff8000')).toEqual([255, 128, 0]);
    expect(channels(' #FF8000 ')).toEqual([255, 128, 0]);
    expect(channels('rosso')).toEqual([0, 0, 0]);
    expect(channels('#fff')).toEqual([0, 0, 0]);
  });

  it('puts black and white at the ends of the scale', () => {
    expect(luminance('#000000')).toBe(0);
    expect(luminance('#ffffff')).toBeCloseTo(1, 5);
  });

  it('measures contrast the way the accessibility rules do', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 1);
    expect(contrast('#258ac0', '#258ac0')).toBeCloseTo(1, 5);
  });

  it('lays a colour over another one at the weight it is drawn with', () => {
    expect(over('#ffffff', 0, '#000000')).toBe('#000000');
    expect(over('#ffffff', 1, '#000000')).toBe('#ffffff');
    expect(over('#ffffff', 0.5, '#000000')).toBe('#808080');
    expect(over('#ffffff', 4, '#000000')).toBe('#ffffff');
    expect(over('#ffffff', -1, '#000000')).toBe('#000000');
  });

  it.each([
    ['#101010', LIGHT_INK],
    ['#ffffff', DARK_INK],
    ['#0b2a3c', LIGHT_INK],
    ['#f0c479', DARK_INK],
  ])('picks the ink that can be read over %s', (background, expected) => {
    expect(readableInk(background)).toBe(expected);
  });

  it.each(['#000000', '#ffffff', '#258ac0', '#f0c479', '#3fd39b', '#7a7a7a'])(
    'keeps %s above the readable threshold',
    (background) => {
      expect(contrast(readableInk(background), background)).toBeGreaterThan(3);
    },
  );

  it('outlines the ink with its opposite, for the backgrounds it cannot know', () => {
    expect(inkShadow(LIGHT_INK)).toContain(DARK_INK);
    expect(inkShadow(DARK_INK)).toContain(LIGHT_INK);
  });
});
