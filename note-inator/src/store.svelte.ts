import type { Settings } from './api';

export const app = $state<{ settings: Settings }>({
  settings: { idle_minutes: 5, lock_on_minimize: false, theme: 'system' },
});
