import { defineConfig } from 'vite';
import path from 'node:path';

// logos 直接用 render 包的素材，避免复制
export default defineConfig({
  publicDir: path.resolve(import.meta.dirname, '../../packages/render/assets/logos'),
  clearScreen: false,
  server: { port: 5183, strictPort: true },
  build: { target: 'safari15' },
});
