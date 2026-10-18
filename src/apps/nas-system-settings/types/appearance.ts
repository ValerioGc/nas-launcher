/**
 * Types for appearance settings.
 */
export interface DesktopBackground {
  mode: 'image' | 'gradient';
  image: string;
  blur: number;
  intensity: number;
  color: string;
  direction: number;
  pattern: 'halo' | 'dots' | 'multiple' | 'linear' | 'rings' | 'grid' | 'stripes';
  size: number;
}
