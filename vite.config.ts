import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Config aligned with Tauri: fixed port 1420, no browser auto-open.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
