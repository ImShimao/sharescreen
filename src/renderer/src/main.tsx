import { createRoot } from 'react-dom/client';
import { App } from './App';
import { createBackend } from './lib/backend';
import './styles.css';

// Pont vers Tauri, sauf si un aperçu a déjà fourni son propre window.sharescreen.
window.sharescreen ??= createBackend();

createRoot(document.getElementById('root')!).render(<App />);
