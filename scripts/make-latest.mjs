// Crée latest.json à partir de l'installeur signé : ce fichier est joint à la release GitHub
// et lu par l'app pour proposer la mise à jour.
// Usage : npm run release (après avoir défini la clé de signature). Le dépôt vient de plugins.updater.endpoints.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const conf = JSON.parse(readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const repo = new URL(conf.plugins.updater.endpoints[0]).pathname.split('/').slice(1, 3).join('/');
if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) {
  console.error('Dépôt introuvable dans plugins.updater.endpoints (attendu : https://github.com/OWNER/REPO/...).');
  process.exit(1);
}
const version = conf.version;
const dir = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
const archive = `ShareScreen_${version}_x64-setup.exe`;
const signature = path.join(dir, `${archive}.sig`);
if (!existsSync(path.join(dir, archive)) || !existsSync(signature)) {
  console.error(`Introuvable : ${archive} et sa signature (.sig) dans ${dir}.`);
  console.error('Lancez npm run build avec la clé de signature (voir README).');
  process.exit(1);
}

const latest = {
  version,
  notes: `ShareScreen ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: readFileSync(signature, 'utf8').trim(),
      url: `https://github.com/${repo}/releases/download/v${version}/${archive}`,
    },
  },
};
writeFileSync(path.join(dir, 'latest.json'), JSON.stringify(latest, null, 2) + '\n');
console.log(`latest.json écrit : ${path.join(dir, 'latest.json')}`);
console.log(`À publier avec la release v${version} : l'installeur, ${archive} et latest.json.`);
