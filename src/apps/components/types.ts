import type { Component } from 'vue';

// Types for the app sections used in the application.
export interface AppSection {
  id: string;
  label: string;
  icon?: string;
  component: Component;
}
