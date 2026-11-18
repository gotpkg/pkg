---
default: minor
---

# pkg update updates mise first

With no package names, pkg update now runs mise self-update before updating packages and tools, and pkg outdated lists a newer mise. A mise installed by Homebrew or another package manager is left to that package manager and listed as skipped. pkg finds mise on PATH, else in ~/.local/bin, and pkg context show works without mise.
