import { useEffect, useState, type ReactNode, type SVGProps } from 'react';
import type { DeviceInfo, IosStatus } from '../../../shared/types';
import {
  IconBack,
  IconCamera,
  IconCheck,
  IconFiles,
  IconHome,
  IconMuted,
  IconPhone,
  IconRecents,
  IconRecord,
  IconRotate,
  IconSound,
  IconStop,
  IconTablet,
  IconVolumeDown,
  IconVolumeUp,
  IconWarn,
} from './icons';

export function DeviceIcon({ kind, ...rest }: { kind: DeviceInfo['kind'] } & SVGProps<SVGSVGElement>) {
  return kind === 'ios' ? <IconPhone {...rest} /> : <IconTablet {...rest} />;
}

const STATE_LABEL: Record<DeviceInfo['state'], string> = {
  ready: 'Prête',
  unauthorized: 'Autorisation',
  offline: 'Hors ligne',
  untrusted: 'À appairer',
  unavailable: 'Indisponible',
};

export function DeviceSwitcher({
  devices,
  activeKey,
  connecting,
  onPick,
}: {
  devices: DeviceInfo[];
  activeKey: string;
  connecting: boolean;
  onPick: (device: DeviceInfo) => void;
}) {
  if (devices.length === 0) {
    return <div className="switcher-empty">Aucun appareil détecté</div>;
  }
  return (
    <div className="switcher" role="radiogroup" aria-label="Appareil">
      {devices.map((d) => {
        const key = `${d.kind}:${d.id}`;
        const active = key === activeKey;
        return (
          <button
            key={key}
            role="radio"
            aria-checked={active}
            className={`device-chip${active ? ' is-active' : ''}`}
            data-state={d.state}
            disabled={connecting && !active}
            title={[d.name, d.detail, d.hint].filter(Boolean).join(' · ') || STATE_LABEL[d.state]}
            onClick={() => onPick(d)}
          >
            <DeviceIcon kind={d.kind} width={17} height={17} />
            <span className="chip-name">{d.name}</span>
            <span className="chip-state">
              <i aria-hidden="true" />
              {STATE_LABEL[d.state]}
            </span>
          </button>
        );
      })}
    </div>
  );
}

interface ChecklistRow {
  ok: boolean;
  label: string;
  hint: string;
  action?: { label: string; run: () => void };
}

/** Checklist iPhone : une ligne par condition, avec l'action qui la règle quand c'est possible. */
export function IosChecklist({
  status,
  onPair,
  onReveal,
  onTunnel,
}: {
  status: IosStatus | null;
  onPair: () => void;
  onReveal: () => void;
  onTunnel: () => void;
}) {
  if (!status) return <p className="muted">Vérification de l'iPhone…</p>;
  if (!status.ios) return <p className="notice error">{status.message}</p>;

  const rows: ChecklistRow[] = [
    {
      ok: status.paired,
      label: 'Appairage',
      hint: 'Acceptez « Faire confiance » sur l\'iPhone.',
      action: status.paired ? undefined : { label: 'Demander l\'appairage', run: onPair },
    },
  ];
  if (status.developerMode !== null) {
    rows.push({
      ok: status.developerMode,
      label: 'Mode développeur',
      hint: 'Réglages > Confidentialité et sécurité > Mode développeur. Si l\'option est absente, révélez-la.',
      action: status.developerMode ? undefined : { label: 'Révéler l\'option', run: onReveal },
    });
  }
  if (status.tunnelRequired) {
    rows.push({
      ok: status.tunnel,
      label: 'Tunnel iOS 17 et plus',
      hint: 'Demande les droits administrateur, une seule fois.',
      action: status.tunnel || !status.paired ? undefined : { label: 'Activer le tunnel', run: onTunnel },
    });
  }
  rows.push({
    ok: status.wda,
    label: 'Contrôle (WebDriverAgent)',
    hint: 'Facultatif. Sans lui, l\'iPhone s\'affiche en lecture seule.',
  });

  return (
    <>
      <ul className="checklist">
        {rows.map((r) => (
          <li key={r.label} className={r.ok ? 'ok' : ''}>
            <span className="check">{r.ok ? <IconCheck width={14} height={14} /> : <i />}</span>
            <div>
              <strong>{r.label}</strong>
              {!r.ok && <small>{r.hint}</small>}
            </div>
            {r.action && (
              <button className="btn small" onClick={r.action.run}>
                {r.action.label}
              </button>
            )}
          </li>
        ))}
      </ul>
      {status.message && <p className="muted small-text">{status.message}</p>}
    </>
  );
}

export function EmptyState({
  selected,
  phase,
  note,
  iosStatus,
  onPair,
  onReveal,
  onTunnel,
  onRetry,
}: {
  selected: DeviceInfo | null;
  phase: 'idle' | 'connecting' | 'error';
  note: string;
  iosStatus: IosStatus | null;
  onPair: () => void;
  onReveal: () => void;
  onTunnel: () => void;
  onRetry: () => void;
}) {
  if (phase === 'connecting') {
    return (
      <div className="empty">
        <div className="spinner" aria-hidden="true" />
        <h2>Connexion en cours</h2>
        <p className="muted">{note || 'Préparation de l\'appareil…'}</p>
      </div>
    );
  }
  if (!selected) {
    return (
      <div className="empty">
        <div className="empty-art" aria-hidden="true">
          <IconTablet width={54} height={54} />
        </div>
        <h2>Choisissez un appareil</h2>
        <p className="muted">Sélectionnez un appareil en haut. Un seul est affiché à la fois, et plusieurs peuvent être branchés.</p>
        <ol className="steps">
          <li>
            <strong>Android</strong>
            Branchez-le en USB, activez le débogage USB dans les options pour développeurs, puis acceptez l'autorisation sur l'écran. Android 5.0 minimum ; le son demande Android 11.
          </li>
          <li>
            <strong>iPhone</strong>
            Branchez-le, déverrouillez-le, acceptez « Faire confiance », puis activez le Mode développeur. iOS 17 et plus demande aussi le tunnel (droits administrateur).
          </li>
        </ol>
      </div>
    );
  }
  if (selected.kind === 'ios') {
    return (
      <div className="empty">
        <div className="empty-art" aria-hidden="true">
          <IconPhone width={54} height={54} />
        </div>
        <h2>{selected.name}</h2>
        {phase === 'error' && <p className="notice error">{note}</p>}
        <IosChecklist status={iosStatus} onPair={onPair} onReveal={onReveal} onTunnel={onTunnel} />
        <button className="btn" onClick={onRetry}>Réessayer</button>
      </div>
    );
  }
  return (
    <div className="empty">
      <div className="empty-art" aria-hidden="true">
        {phase === 'error' ? <IconWarn width={54} height={54} /> : <IconTablet width={54} height={54} />}
      </div>
      <h2>{selected.name}</h2>
      <p className={phase === 'error' ? 'notice error' : 'muted'}>{note || selected.detail || 'Prête à être affichée.'}</p>
      {selected.hint && <p className="hint">{selected.hint}</p>}
      <button className="btn primary" onClick={onRetry}>{phase === 'error' ? 'Réessayer' : 'Afficher'}</button>
    </div>
  );
}

export interface ToolbarState {
  kind: DeviceInfo['kind'];
  /** Image affichée : capture, enregistrement et fichiers. */
  live: boolean;
  /** Contrôle disponible : touches et navigation. */
  canInput: boolean;
  audioOn: boolean;
  recording: boolean;
  onKey: (keycode: number) => void;
  onHome: () => void;
  onRotate: () => void;
  onAudio: () => void;
  onScreenshot: () => void;
  onRecord: () => void;
  onFiles: () => void;
  onStop: () => void;
}

export function Toolbar(s: ToolbarState) {
  const android = s.kind === 'android';
  return (
    <nav className="rail" aria-label="Commandes">
      {android ? (
        <>
          <Tool label="Retour" onClick={() => s.onKey(4)} disabled={!s.canInput}><IconBack /></Tool>
          <Tool label="Accueil" onClick={() => s.onKey(3)} disabled={!s.canInput}><IconHome /></Tool>
          <Tool label="Applis récentes" onClick={() => s.onKey(187)} disabled={!s.canInput}><IconRecents /></Tool>
          <div className="rail-sep" />
          <Tool label="Volume +" onClick={() => s.onKey(24)} disabled={!s.canInput}><IconVolumeUp /></Tool>
          <Tool label="Volume −" onClick={() => s.onKey(25)} disabled={!s.canInput}><IconVolumeDown /></Tool>
          <Tool label="Rotation" onClick={s.onRotate} disabled={!s.canInput}><IconRotate /></Tool>
          <div className="rail-sep" />
          <Tool label={s.audioOn ? 'Couper le son' : 'Activer le son'} onClick={s.onAudio} active={s.audioOn}>
            {s.audioOn ? <IconSound /> : <IconMuted />}
          </Tool>
        </>
      ) : (
        <>
          <Tool label="Accueil de l'iPhone" onClick={s.onHome} disabled={!s.canInput}><IconHome /></Tool>
          <div className="rail-sep" />
        </>
      )}
      <Tool label="Capture d'écran" onClick={s.onScreenshot} disabled={!s.live}><IconCamera /></Tool>
      <Tool
        label={s.recording ? 'Arrêter l\'enregistrement' : 'Enregistrer la vidéo'}
        onClick={s.onRecord}
        active={s.recording}
        danger={s.recording}
        disabled={!s.live}
      >
        {s.recording ? <IconStop /> : <IconRecord />}
      </Tool>
      {android && <Tool label="Fichiers" onClick={s.onFiles} disabled={!s.live}><IconFiles /></Tool>}
      <div className="rail-spacer" />
      <Tool label="Arrêter l'affichage" onClick={s.onStop} danger>
        <IconStop />
      </Tool>
    </nav>
  );
}

function Tool({
  label,
  onClick,
  disabled,
  active,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      className={`tool${active ? ' is-active' : ''}${danger ? ' is-danger' : ''}`}
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
    >
      {children}
    </button>
  );
}

export interface Toast {
  id: number;
  text: string;
  tone: 'info' | 'ok' | 'error';
}

export function Toasts({ toasts }: { toasts: Toast[] }) {
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.tone}`}>
          {t.text}
        </div>
      ))}
    </div>
  );
}

export function FilesSheet({
  serial,
  onClose,
  onPickFiles,
  onNotify,
}: {
  serial: string;
  onClose: () => void;
  onPickFiles: () => void;
  onNotify: (text: string, tone: Toast['tone']) => void;
}) {
  const [files, setFiles] = useState<string[] | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    let alive = true;
    window.sharescreen
      .listFiles(serial)
      .then((list) => {
        if (alive) setFiles(list);
      })
      .catch((err: unknown) => {
        if (alive) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      alive = false;
    };
  }, [serial]);

  const pull = async (name: string) => {
    try {
      const saved = await window.sharescreen.pullFile(serial, name);
      if (saved) onNotify(`« ${name} » enregistré.`, 'ok');
    } catch (err) {
      onNotify(err instanceof Error ? err.message : String(err), 'error');
    }
  };

  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <section className="sheet" role="dialog" aria-label="Fichiers" onClick={(e) => e.stopPropagation()}>
        <header>
          <h3>Fichiers</h3>
          <button className="btn ghost" onClick={onClose}>Fermer</button>
        </header>
        <div className="sheet-actions">
          <button className="btn primary" onClick={onPickFiles}>Envoyer vers l'appareil</button>
          <small className="muted">Les fichiers arrivent dans Téléchargements.</small>
        </div>
        <h4>Depuis l'appareil · Téléchargements</h4>
        {error && <p className="notice error">{error}</p>}
        {!files && !error && <p className="muted">Lecture du dossier…</p>}
        {files && files.length === 0 && <p className="muted">Dossier vide.</p>}
        <ul className="file-list">
          {files?.map((name) => (
            <li key={name}>
              <span className="file-name">{name}</span>
              {!name.endsWith('/') && <button className="btn" onClick={() => pull(name)}>Enregistrer</button>}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
