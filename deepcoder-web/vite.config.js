import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const websocketProxy = {
  '/ws': {
    target: process.env.DEEPCODER_WS_PROXY_TARGET || 'ws://127.0.0.1:8080',
    ws: true,
  },
}

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    host: true,
    proxy: websocketProxy,
  },
  preview: {
    port: 4173,
    host: true,
    proxy: websocketProxy,
  }
})
