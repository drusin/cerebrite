// Ticket 13 (app shell): the whole frontend is one Vue app now -- this file
// is just the bootstrap. Everything that used to live here (every
// `mountIsland` call, the workspace/sidebar chrome, the global keyboard
// shortcuts, `init()`) moved into `App.vue` (see its own doc comment) and
// the surface containers it assembles.
import { createApp } from "vue";
import App from "./App.vue";

createApp(App).mount("#app");
