import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Config alineada con Tauri: puerto fijo 1420, sin abrir navegador.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
