import { useEffect, useState } from 'react';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

/**
 * Propose une mise à jour au lancement. « Mettre à jour » télécharge et installe la nouvelle version
 * puis redémarre l'app ; « Plus tard » la masque jusqu'au prochain lancement.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    let alive = true;
    // Hors ligne ou aucune version publiée : on reste silencieux.
    check()
      .then((found) => {
        if (alive && found) setUpdate(found);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  if (!update || dismissed) return null;

  const install = async () => {
    let total = 0;
    let done = 0;
    setProgress(0);
    setError('');
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          done += event.data.chunkLength;
          if (total > 0) setProgress(Math.min(100, Math.round((done / total) * 100)));
        }
      });
      await relaunch();
    } catch (err) {
      setProgress(null);
      setError(`Mise à jour impossible : ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="update-banner" role="status">
      <span>Version {update.version} disponible.</span>
      {progress === null ? (
        <>
          <button className="btn primary small" onClick={() => void install()}>
            Mettre à jour
          </button>
          <button className="btn ghost small" onClick={() => setDismissed(true)}>
            Plus tard
          </button>
        </>
      ) : (
        <span>Téléchargement {progress} %…</span>
      )}
      {error && <span className="notice error">{error}</span>}
    </div>
  );
}
