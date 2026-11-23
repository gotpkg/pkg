---
default: patch
---

# Show only the tools pkg update installs, not every version mise looks up

In a terminal, pkg update left one line on stderr for every tool in your mise config, even when you named a single package, and its display counted them as tools being updated ("0/67 tools"). Those lines only report mise looking versions up before it upgrades anything. They now update the display and disappear; a lookup that fails still stays on stderr. Piped output, --output json and CI runs still get every line mise prints.
