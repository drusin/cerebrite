# Git-native markdown as the permanent source of truth

Logseq, the direct inspiration for this project, deprecated its markdown-based file format in favor of a database-backed one — leaving users of the file-based version stranded. Cerebrite decides upfront that plain markdown files, synced via git, are the permanent source of truth, never an export format or a migration path to something else. Every other architectural choice must preserve this: no feature may require storing state that isn't a markdown file under version control.
