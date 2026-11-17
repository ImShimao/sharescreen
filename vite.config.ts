import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const pkg = JSON.parse(readFileSync(resolve('package.json'), 'utf8')) as { version: string };

// Interface React servie par Vite en développement, puis embarquée par Tauri.
export default defineConfig({
  root: resolve('src/renderer'),
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: resolve('dist'),
    emptyOutDir: true,
    target: 'chrome130',
  },
});
