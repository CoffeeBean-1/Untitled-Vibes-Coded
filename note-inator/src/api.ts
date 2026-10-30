import { invoke } from '@tauri-apps/api/core';

export type Kind = 'notebook' | 'section' | 'page';
export interface PageMeta { id: string; title: string; modified: number }
export interface Section { id: string; name: string; pages: PageMeta[] }
export interface Notebook { id: string; name: string; sections: Section[] }
export interface Settings {
  idle_minutes: number | null;
  lock_on_minimize: boolean;
  theme: 'system' | 'light' | 'dark';
}
export interface PageFull { id: string; title: string; content: unknown }

export const api = {
  status: () => invoke<{ exists: boolean; unlocked: boolean }>('vault_status'),
  createVault: (password: string) => invoke<Settings>('create_vault', { password }),
  unlock: (password: string) => invoke<Settings>('unlock', { password }),
  lock: (minimize = false) => invoke<void>('lock_vault', { minimize }),
  quit: () => invoke<void>('quit_app'),
  activity: () => invoke<void>('activity'),
  getTree: () => invoke<Notebook[]>('get_tree'),
  getPage: (id: string) => invoke<PageFull>('get_page', { id }),
  savePage: (id: string, title: string, content: unknown) =>
    invoke<PageMeta>('save_page', { id, title, content }),
  createNode: (kind: Kind, parentId: string, name: string) =>
    invoke<string>('create_node', { kind, parentId, name }),
  renameNode: (kind: Kind, id: string, name: string) => invoke<void>('rename_node', { kind, id, name }),
  deleteNode: (kind: Kind, id: string) => invoke<void>('delete_node', { kind, id }),
  reorder: (kind: Kind, parentId: string, ids: string[]) =>
    invoke<void>('reorder_nodes', { kind, parentId, ids }),
  movePage: (pageId: string, sectionId: string) => invoke<void>('move_page', { pageId, sectionId }),
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<void>('set_settings', { settings }),
  changePassword: (oldPassword: string, newPassword: string) =>
    invoke<void>('change_password', { oldPassword, newPassword }),
  exportBackup: (dest: string) => invoke<void>('export_backup', { dest }),
  importBackup: (src: string) => invoke<void>('import_backup', { src }),
};

export const errMsg = (e: unknown) => (typeof e === 'string' ? e : (e as Error)?.message ?? String(e));
