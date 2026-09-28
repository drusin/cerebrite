# Framework or not: how surfaces are built

Type: grilling
Status: open
Blocked by: 02, 03

## Question

Given the shape options (ticket 02) and the coupling picture (ticket 03), are surfaces built as plain TS render modules, web components, or Vue 3 components?

The standing lean is "no hard opinion, Vue 3 if a framework". The decision must weigh the migration cost against how much simpler isolation, stories, and future tests become. This is hard to reverse and likely ADR-worthy.
