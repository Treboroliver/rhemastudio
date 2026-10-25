import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    svelte(),
    tailwindcss(),
  ],
  resolve: {
    conditions: ['browser'],
  },
  ssr: {
    resolve: {
      conditions: ['browser'],
    },
  },
  test: {
    environment: 'jsdom',
    environmentOptions: {
      jsdom: {
        customExportConditions: ['browser'],
      },
    },
    include: ['src/**/*.test.ts'],
  },
})
