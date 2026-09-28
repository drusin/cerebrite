# Pilot surface and migration order

Type: grilling
Status: open
Blocked by: 07

## Question

Given the [Surface contract](07-surface-contract.md), which surface is the pilot, and in what order do the rest migrate out of `main.ts`, keeping the app working after every step?

- **Pilot:** the sync indicator + popup is the leading candidate. Its derivation `syncIndicatorFor` is pure, and after the contract only its anchor positioning is left to cut. Search and the vault picker are similarly isolated. The editor is isolated but a leaf. Decide which proves the most about the contract (surface/container split, a state module, `mountIsland`, callback root props, stories) for the least risk.
- **Order:** where the **app shell** step (the islands merge into one `App.vue`) falls, how the tangled page-navigation cluster (page list, Recent, Trash, article + backlinks, editor glue) and the Settings hub are approached, and whether they are split into smaller steps.
- **Grouping by shared components:** how the extract-on-second-copy rule (device flow, SSH key, Commit as, credential-kind, page list, `<Modal>`) shapes which surfaces go next to each other.
- **Per-step definition of done:** the Vue SFC plus its stories, the container, the state it moved out of `main.ts`, and the callback root props it deleted. Also any per-step checks (`vue-tsc`, `storybook build`, a manual run of the real app).

Background: the coupling summary in the [surface inventory](../research/03-surface-inventory.md).
