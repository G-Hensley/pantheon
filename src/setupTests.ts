import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// vitest.config.ts does not turn on `test.globals`, so afterEach/cleanup
// needs to be wired explicitly rather than relying on @testing-library's
// auto-cleanup (which only registers when `afterEach` is already global).
afterEach(() => {
  cleanup();
});
