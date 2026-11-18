---
default: minor
---

# Set up a new machine with pkg setup

pkg setup installs mise with mise's own installer when none is found, puts mise's shims on PATH when no mise config exists yet, and hands pkg to mise: it declares github:gotpkg/pkg and links ~/.local/bin/pkg to mise's shim, so pkg update keeps pkg current from then on. pkg context show reports who installed the pkg you are running.
