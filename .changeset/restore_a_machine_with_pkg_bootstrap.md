---
default: minor
---

# Restore a machine with pkg bootstrap

pkg bootstrap runs mise bootstrap for your global configuration, a dotfiles repository (--from, --directory, --config-dir), or one config file (--config). It installs mise when it is missing, refuses before changing anything when ~/.config/mise already holds another configuration, moves the installer's PATH lines aside while your dotfiles deploy their own startup files, lets mise ask for passwords and answers in a terminal, and hands pkg to mise when it succeeds. --dry-run previews without running hooks or templated files.
