import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Separate from vite.config.ts on purpose: the app config is tailored for
// `tauri dev`/`tauri build` (fixed port, ignoring src-tauri, etc.), none of
// which a unit test run needs or should depend on.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/setupTests.ts"],
    css: false,
  },
});
