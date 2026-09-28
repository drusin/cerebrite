import type { StorybookConfig } from "@storybook/vue3-vite";

// Foundation ticket (01) / spec.md#storybook. This is a dev-only workbench:
// it never runs inside the Tauri webview and is never part of the app
// bundle (nothing under `src/` that the app imports may import a
// `*.stories.ts` file or `@storybook/*`).
const config: StorybookConfig = {
  stories: ["../src/**/*.stories.ts"],
  addons: ["@storybook/addon-themes"],
  framework: {
    name: "@storybook/vue3-vite",
    options: {},
  },
  core: {
    disableTelemetry: true,
  },
  // Same as vite.config.ts: without this, Vite's watcher trips over
  // src-tauri/target's churning incremental build artifacts (a concurrent
  // `cargo build` can rotate/delete them mid-`lstat`), which crashes the
  // dev server with an unhandled EBADF.
  async viteFinal(config) {
    config.server ??= {};
    config.server.watch ??= {};
    const ignored = config.server.watch.ignored ?? [];
    config.server.watch.ignored = [...(Array.isArray(ignored) ? ignored : [ignored]), "**/src-tauri/**"];
    return config;
  },
};

export default config;
