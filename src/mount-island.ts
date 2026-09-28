// See spec.md#islands-and-how-they-merge: a thin wrapper around
// `createApp(...).mount(el)`. There is deliberately no framework-neutral
// `{update, destroy}` handle -- callers that need to unmount an island can
// call `app.unmount()` on the `App` this returns.
import { createApp, type App, type Component } from "vue";

export function mountIsland(
  el: Element | string,
  Container: Component,
  rootProps?: Record<string, unknown>,
): App {
  const app = createApp(Container, rootProps);
  app.mount(el);
  return app;
}
