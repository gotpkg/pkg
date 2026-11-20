---
default: patch
---

# Make pkg links shareable without breaking the installer

The homepage serves HTML to sharing crawlers and other clients by default. Standard curl and GNU Wget requests still receive the installer; custom download clients can use /install.sh to request it explicitly.
