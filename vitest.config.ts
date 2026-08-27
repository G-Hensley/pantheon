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
    // src/App.css.test.ts imports App.css as text to guard the stylesheet
    // against the top-of-window scrollbar regression. `css: false` stubs every
    // CSS id, including the `?raw` query, so that import would come back as an
    // empty string and the whole guard would pass while checking nothing. No
    // component under test imports a stylesheet, so this costs one file.
    css: true,
  },
});
