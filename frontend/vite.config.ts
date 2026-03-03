import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { paraglideVitePlugin } from '@inlang/paraglide-js';
import path from 'path';

export default defineConfig({
  plugins: [
    svelte(),
    paraglideVitePlugin({
      project: './project.inlang',
      outdir: './src/lib/paraglide',
    }),
  ],
  resolve: {
    alias: {
      $lib: path.resolve(__dirname, './src/lib'),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        ws: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      onwarn(warning, warn) {
        // Suppress node:* externalization warnings emitted by paraglide's
        // build-tool internals (compiler, unplugin) — they are not part of
        // the browser runtime bundle and cause no runtime issues.
        if (warning.message.includes('has been externalized for browser compatibility')) return;
        warn(warning);
      },
      output: {
        manualChunks: {
          vendor: ['svelte-spa-router', '@inlang/paraglide-js'],
        },
      },
    },
  },
});
