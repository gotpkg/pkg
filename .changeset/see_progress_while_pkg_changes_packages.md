---
default: minor
---

# See progress while pkg changes packages

In a terminal, pkg install, update, sync, remove and prune now show a live display while they work: the step pkg is on and how long it has taken, the package being worked on and how many remain, every formula, cask or tool mise is working on with its last phase, and a bar wherever mise has given a total, such as the formulae a package brings or a tool download's bytes. One line per finished item stays on stderr afterwards, and a failure still leaves every line mise printed. To count formulae, pkg install asks mise for each Homebrew package's plan first, which takes a fraction of a second. With stderr redirected, with --output json, or with CI set, output is exactly as before.

For pkg-core users, Progress gains Step and Plan events and InstallRequest gains report_plans. Code that matches every Progress event or builds an InstallRequest needs the new arm and field; ignoring the new events loses nothing.
