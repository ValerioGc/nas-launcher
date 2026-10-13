import { describe, expect, it } from 'vitest';
import { backgroundStyle } from '@/apps/nas-system-settings/background';
import type { DesktopBackground } from '@/apps/nas-system-settings/types/appearance';

const sample: DesktopBackground = {
  mode: 'gradient',
  image: '/wallpapers/landscape.svg',
  blur: 0,
  intensity: 40,
  color: '#258ac0',
  direction: 90,
  pattern: 'halo',
  size: 60,
};

describe('desktop backgrounds', () => {
  it.each(['halo', 'dots', 'multiple', 'linear'] as const)(
    'renders %s without external assets',
    (pattern) => {
      expect(backgroundStyle({ ...sample, pattern }).backgroundImage).toContain('gradient(');
    },
  );

  it('bounds values and refuses arbitrary image URLs', () => {
    const style = backgroundStyle({
      ...sample,
      mode: 'image',
      image: 'https://tracking.invalid/a.jpg',
      blur: 999,
      intensity: 200,
    });
    expect(style.backgroundImage).toBe('url("/wallpapers/landscape.svg")');
    expect(style.filter).toBe('blur(30px)');
    expect(style.opacity).toBe(1);
  });

  it('does not inject malformed colors into CSS', () => {
    expect(
      backgroundStyle({ ...sample, color: 'red);url(https://bad)' }).backgroundImage,
    ).not.toContain('bad');
  });
});
