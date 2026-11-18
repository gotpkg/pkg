---
default: minor
---

# Install pkg with one command

curl -fsSL https://gotpkg.dev | sh installs pkg at ~/.local/bin/pkg on macOS and Linux without a compiler or another package manager, checks the download against the release's SHA256SUMS, puts ~/.local/bin on PATH in your shell's startup file, and prints the command that loads it in the current terminal. Every release now carries install.sh and SHA256SUMS.
