// Tiny promise-based dialogs (window.prompt/confirm are unreliable in webviews).
interface Req {
  kind: 'confirm' | 'prompt';
  message: string;
  value: string;
  resolve: (v: any) => void;
}
export const dlg = $state<{ current: Req | null }>({ current: null });

export const confirmBox = (message: string) =>
  new Promise<boolean>((resolve) => { dlg.current = { kind: 'confirm', message, value: '', resolve }; });

export const promptBox = (message: string, value = '') =>
  new Promise<string | null>((resolve) => { dlg.current = { kind: 'prompt', message, value, resolve }; });
