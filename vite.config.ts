import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// Interface React servie par Vite en développement, puis embarquée par Tauri.
export default defineConfig({
  root: resolve('src/renderer'),
  plugins: [react()],
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
