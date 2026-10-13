import { createPinia } from 'pinia';

/**
 * The shared Pinia instance: `main.ts` installs it in the application,
 * and the specs activate it with `setActivePinia`.
 */
export const pinia = createPinia();
