/** Touches spéciales transmises à Android (codes KeyEvent.KEYCODE_*). */
const SPECIAL: Record<string, number> = {
  Enter: 66,
  Backspace: 67,
  Delete: 112,
  Tab: 61,
  Escape: 4, // retour Android
  ArrowUp: 19,
  ArrowDown: 20,
  ArrowLeft: 21,
  ArrowRight: 22,
  Home: 122,
  End: 123,
  PageUp: 92,
  PageDown: 93,
};

// Indicateurs de modificateurs (KeyEvent.META_*)
const META_SHIFT_ON = 1;
const META_ALT_ON = 2;
const META_CTRL_ON = 4096;

export function keycodeFor(key: string): number | undefined {
  return SPECIAL[key];
}

export function metaFor(e: { shiftKey: boolean; altKey: boolean; ctrlKey: boolean }): number {
  return (e.shiftKey ? META_SHIFT_ON : 0) | (e.altKey ? META_ALT_ON : 0) | (e.ctrlKey ? META_CTRL_ON : 0);
}
