import { getCurrentWindow } from '@tauri-apps/api/window';

/** Boutons de fenêtre dessinés par l'app (la barre native est masquée pour l'effet Mica). */
export function WindowControls() {
  // La fenêtre n'est lue qu'au clic : l'interface reste utilisable hors Tauri (aperçu).
  const run = (action: (w: ReturnType<typeof getCurrentWindow>) => Promise<void>) => () => {
    void action(getCurrentWindow()).catch(() => undefined);
  };
  return (
    <div className="win-controls">
      <button className="win-btn" aria-label="Réduire" title="Réduire" onClick={run((w) => w.minimize())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" stroke="currentColor" strokeWidth="1" /></svg>
      </button>
      <button className="win-btn" aria-label="Agrandir" title="Agrandir" onClick={run((w) => w.toggleMaximize())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
      </button>
      <button className="win-btn is-close" aria-label="Fermer" title="Fermer" onClick={run((w) => w.close())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1" /></svg>
      </button>
    </div>
  );
}
