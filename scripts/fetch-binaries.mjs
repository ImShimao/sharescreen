// Télécharge les outils externes dans resources/bin (exécuté automatiquement par npm install).
// Chaque archive est épinglée à une version précise et vérifiée par son SHA-256 avant usage.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { copyFileSync, createReadStream, createWriteStream, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import https from 'node:https';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = path.join(root, 'resources', 'bin');

// name: archive ou fichier téléchargé ; files: membre de l'archive -> nom dans resources/bin.
const ARTIFACTS = [
  {
    name: 'adb (platform-tools 37.0.1, Google, Apache-2.0)',
    url: 'https://dl.google.com/android/repository/platform-tools_r37.0.1-win.zip',
    sha256: '45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99',
    files: {
      'platform-tools/adb.exe': 'adb.exe',
      'platform-tools/AdbWinApi.dll': 'AdbWinApi.dll',
      'platform-tools/AdbWinUsbApi.dll': 'AdbWinUsbApi.dll',
    },
  },
  {
    name: 'go-ios 1.3.2 (MIT)',
    url: 'https://github.com/danielpaulus/go-ios/releases/download/v1.3.2/go-ios-win.zip',
    sha256: '939c6bcaafed183a92afb9f79cc11b1f935fa6389bfc94d3902e3f52c4dff3fe',
    files: { 'ios.exe': 'ios.exe' },
  },
  {
    name: 'serveur scrcpy 5.0.1 (Apache-2.0)',
    url: 'https://github.com/Genymobile/scrcpy/releases/download/v5.0.1/scrcpy-server-v5.0.1',
    sha256: '764eb6f79811d5211fe9df341120882ba9994c7a61b897d7bf3fb662e53bc536',
    direct: 'scrcpy-server.jar',
  },
  {
    name: 'wintun 0.14.1 (tunnel iOS 17+)',
    url: 'https://www.wintun.net/builds/wintun-0.14.1.zip',
    sha256: '07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51',
    files: { 'wintun/bin/amd64/wintun.dll': 'wintun.dll' },
  },
];

function download(url, dest, redirects = 5) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if ([301, 302, 303, 307, 308].includes(res.statusCode)) {
          res.resume();
          if (redirects === 0) return reject(new Error(`trop de redirections pour ${url}`));
          return resolve(download(new URL(res.headers.location, url).toString(), dest, redirects - 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} pour ${url}`));
        }
        const file = createWriteStream(dest);
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      })
      .on('error', reject);
  });
}

function sha256(file) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256');
    createReadStream(file)
      .on('data', (chunk) => hash.update(chunk))
      .on('end', () => resolve(hash.digest('hex')))
      .on('error', reject);
  });
}

async function fetchArtifact(artifact, tmp) {
  const targets = artifact.direct ? [artifact.direct] : Object.values(artifact.files);
  if (targets.every((t) => existsSync(path.join(bin, t)))) {
    console.log(`  déjà présent : ${artifact.name}`);
    return;
  }
  console.log(`  téléchargement : ${artifact.name}`);
  const archive = path.join(tmp, path.basename(artifact.url));
  await download(artifact.url, archive);
  const actual = await sha256(archive);
  if (actual !== artifact.sha256) {
    throw new Error(`empreinte SHA-256 inattendue pour ${artifact.name} (${actual})`);
  }
  if (artifact.direct) {
    copyFileSync(archive, path.join(bin, artifact.direct));
    return;
  }
  // tar.exe de Windows (10+) sait extraire les zip. Chemin absolu : le tar de Git, plus haut dans le PATH, ne convient pas.
  const tar = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
  const out = path.join(tmp, 'extract');
  mkdirSync(out, { recursive: true });
  for (const [member, target] of Object.entries(artifact.files)) {
    execFileSync(tar, ['-xf', archive, '-C', out, member], { stdio: 'ignore' });
    copyFileSync(path.join(out, member), path.join(bin, target));
  }
}

async function main() {
  if (process.platform !== 'win32') {
    console.log('ShareScreen est une application Windows : rien à télécharger ici.');
    return;
  }
  mkdirSync(bin, { recursive: true });
  const tmp = mkdtempSync(path.join(os.tmpdir(), 'sharescreen-'));
  try {
    console.log('Outils externes (resources/bin) :');
    for (const artifact of ARTIFACTS) {
      await fetchArtifact(artifact, tmp);
    }
    console.log('Outils externes prêts.');
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(`\nÉchec du téléchargement des outils : ${err.message}`);
  console.error('Vérifiez votre connexion Internet puis relancez : npm install');
  process.exit(1);
});
