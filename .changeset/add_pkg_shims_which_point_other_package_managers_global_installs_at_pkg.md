---
default: minor
---

# Add pkg shims, which point other package managers' global installs at pkg

pkg shims install writes small executables named brew, apt, npm, pip, winget and the other package managers mise can drive. With their directory first on PATH, a global install through one runs nothing and names the pkg command that does it, so what an agent installs is recorded in your mise config. Everything else runs the real tool, and the mise shim keeps your own mise bootstrap working. pkg shims list shows what each name runs and prints the PATH line to add; pkg shims remove deletes the shims.
