import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  // Tauri expects a fixed port in dev
  server: {
    port: 1420,
    strictPort: true,
  },
  // Inline env vars Tauri needs
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Tauri supports ES2021
    target: ['es2021', 'chrome105', 'safari15'],
    minify: !process.env.TAURI_DEBUG,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
})
