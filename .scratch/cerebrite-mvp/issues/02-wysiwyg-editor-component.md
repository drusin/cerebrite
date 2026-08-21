Type: research
Status: open
Blocked by: 01

## Question

Given the tech stack chosen in [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md), which WYSIWYG editor component can render and edit markdown for the Windows/Linux desktop apps while emitting "clean" markdown on save — no metadata outside YAML frontmatter, per [ADR-0003](../../../docs/adr/0003-clean-markdown-excludes-round-trip-fidelity.md) (round-trip fidelity is not required, but no stray inline markup may be injected into the body)?

Research candidate libraries within the chosen framework's ecosystem. If none satisfy clean-markdown-out, evaluate the feasibility of a minimal custom editor as a fallback, and estimate the effort gap between adopting an existing library vs. building one.
