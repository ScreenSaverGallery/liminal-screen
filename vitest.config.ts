import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "src/**/*.test.ts",
      "packages/liminal-api/src/**/*.test.ts",
    ],
  },
});
