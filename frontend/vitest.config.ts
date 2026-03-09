import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: {
    alias: {
      '$lib/paraglide/messages.js': path.resolve(__dirname, './src/__mocks__/paraglide-messages.ts'),
      '$lib/paraglide/runtime.js': path.resolve(__dirname, './src/__mocks__/paraglide-runtime.ts'),
      $lib: path.resolve(__dirname, './src/lib'),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    include: ['src/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/lib/**/*.ts'],
      exclude: ['src/lib/paraglide/**', 'src/**/*.test.ts', 'src/test-setup.ts'],
    },
  },
});
