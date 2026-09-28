import type { Preview } from "@storybook/vue3-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import "../src/styles.css";

// Foundation ticket (01) / spec.md#storybook.
const THEME_SYSTEM = "system";
const THEME_LIGHT = "light";
const THEME_DARK = "dark";

const preview: Preview = {
  parameters: {
    viewport: {
      options: {
        phone: {
          name: "Phone",
          styles: { width: "375px", height: "667px" },
          type: "mobile",
        },
        tablet: {
          name: "Tablet",
          styles: { width: "768px", height: "1024px" },
          type: "tablet",
        },
      },
    },
  },
  decorators: [
    withThemeByDataAttribute({
      themes: {
        [THEME_SYSTEM]: THEME_SYSTEM,
        [THEME_LIGHT]: THEME_LIGHT,
        [THEME_DARK]: THEME_DARK,
      },
      defaultTheme: THEME_SYSTEM,
    }),
    // `withThemeByDataAttribute` can only *set* `data-theme`, never remove
    // it, so "System" would otherwise land as `data-theme="system"` instead
    // of no attribute at all. This matches `applyTheme` in main.ts exactly,
    // which removes the attribute for "system" and lets
    // `prefers-color-scheme` apply (see styles.css's
    // `:root:not([data-theme="light"])` dark-mode rule).
    (story, context) => {
      if (context.globals.theme === THEME_SYSTEM) {
        document.documentElement.removeAttribute("data-theme");
      }
      return story();
    },
  ],
};

export default preview;
