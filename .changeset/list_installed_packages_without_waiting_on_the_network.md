---
default: patch
---

# List installed packages without waiting on the network

pkg list, info, install, sync and update no longer wait on mise to fetch every Homebrew cask's definition before saying what is installed. pkg now leaves casks out of mise's check and reads them from Homebrew's Caskroom, so pkg list answers in about 0.2 s instead of 1–3 s and works offline. mise still checks every other package manager, apt included, on the machine. pkg install also stops asking what is installed before it installs a package nothing declares yet.

In JSON output, a package that is not installed now reports its version as null instead of an empty string.
