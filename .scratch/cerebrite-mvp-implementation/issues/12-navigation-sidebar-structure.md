# 12: Navigation/sidebar structure

**What to build:** The full sidebar per [08-navigation-sidebar-ui-structure](../../cerebrite-mvp/issues/08-navigation-sidebar-ui-structure.md): a prominent Search entry point (button; Ctrl/Cmd+K wired once ticket 13 exists), Recent (last-*opened* order, capped at 10, no "show more"), All pages (already built in ticket 02), and the Today shortcut (ticket 11). Desktop gets a persistent, user-collapsible sidebar; Android gets a hamburger-triggered drawer, hidden by default.

**Blocked by:** 10, 11

**Status:** ready-for-agent

- [ ] Sidebar shows sections in order: Search entry point, Recent, All pages, Today
- [ ] Recent tracks last-opened pages (not last-edited), capped at 10, most recent first
- [ ] Dynamic and persisted pages are visually indistinguishable everywhere in the sidebar
- [ ] Desktop (Windows/Linux): sidebar is persistent and user-collapsible
- [ ] Android: sidebar is a hamburger-triggered drawer, hidden by default
