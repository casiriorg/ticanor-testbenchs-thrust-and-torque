import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Port 1420 is what src-tauri/tauri.conf.json expects in development.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2021", outDir: "dist" },
});
