---
id: proposal.0006
title: Shareable Homepage Routing
authors: [Mark]
status: awaiting-implementation
pull-request: https://github.com/gotpkg/pkg/pull/10
issues: []
supersedes: []
---

# Shareable Homepage Routing

## Summary

Serve HTML by default at `gotpkg.dev`, including to sharing crawlers. Preserve the root installer for ordinary curl and wget requests and keep `/install.sh` as the unconditional script endpoint. This makes homepage metadata reachable without maintaining a list of social-media crawlers.

## Motivation

A person sharing pkg in Messages or on a social network expects a preview of the website. Today, a crawler that sends `Accept: */*` or omits `Accept` receives a shell script, even when the site's HTML contains the right metadata. Adding an image cannot fix that response selection.

## Domain grounding

### Established context

- [Apple's Messages guidance](https://developer.apple.com/documentation/technotes/tn3156-create-rich-previews-for-messages) requires metadata in the linked page's initial HTML; previews do not run JavaScript. It recommends consistent metadata across user agents.
- [`Accept: */*`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Accept) permits any media type. It is the default for curl and wget, not evidence that a client wants a shell script.
- [A documented crawler compatibility incident](https://github.com/flancer32/teq-cms/issues/34) reports wildcard requests from Twitterbot and TelegramBot receiving the wrong representation. This is evidence of that failure mode, not a universal specification of crawler headers.

### Relevant constraints and principles

- The [vision](../VISION.md#goals) promises `curl -fsSL https://gotpkg.dev | sh`; that command must keep working.
- User-agent identification is a representation hint, not authentication. The script is already public at `/install.sh`.
- [`Vary`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Vary) must identify every request header used to select the response.

### Quality bar

Unknown sharing clients receive crawlable HTML without a crawler allowlist. Script downloaders have an explicit endpoint that does not depend on identification. Neither representation is cached as the other.

### Remaining uncertainty

Platforms choose their own presentation and caching policies. Correct HTML and reachable artwork cannot guarantee immediate preview refresh on every platform. No particular crawler identity is assumed by this design.

## Existing baseline

The [site Worker](../site/app/worker.js) and [temporary installer Worker](../installer/worker/worker.js) choose HTML only when `Accept` contains `text/html`. The [current routing contract](../README.md#installer-routing) explicitly excludes user-agent selection. The site in [PR #5](https://github.com/gotpkg/pkg/pull/5) supplies prerendered OpenGraph metadata and the shared PNG; the live temporary Worker still supplies its placeholder page.

Preserve the installer bytes, release lookup, cache lifetime, failure responses, explicit script route, and static-asset routing. Change only how the root chooses its representation and how that choice is documented and tested.

## Proposed solution

Make HTML the root's default. Treat the ordinary curl and wget product identifiers as a request for the installer unless the client explicitly requests HTML. Clients that need a script regardless of headers use `/install.sh`.

| Request                                                               | Response  |
| --------------------------------------------------------------------- | --------- |
| `/`, `User-Agent: curl/8.7.1`, `Accept: */*`                          | Installer |
| `/`, `User-Agent: Wget/1.25.0`, no `Accept`                           | Installer |
| `/`, curl or wget, explicit `Accept: text/html`                       | HTML      |
| `/`, a sharing crawler or unknown client, wildcard or absent `Accept` | HTML      |
| `/install.sh`, any user agent or `Accept`                             | Installer |

## Detailed design

### Root representation selection

Revise [README: Installer routing](../README.md#installer-routing) to replace the Accept-only rule with this order:

1. An explicit `text/html` media range with positive quality selects HTML. Match the media type case-insensitively; omitted quality means one. A zero or malformed quality does not establish an HTML override. Wildcards alone are not an explicit HTML request.
2. Otherwise, a first `User-Agent` product of `curl/<version>` or `Wget/<version>` selects the installer. Match these product names case-insensitively and require a nonempty version. A token mentioned later in another client's user-agent string does not qualify.
3. Every other root request selects HTML, including absent headers, custom clients, and non-HTML Accept values. This is a two-representation convenience endpoint, not a general media-negotiation API; it introduces no `406` response.

Apply this rule to the [site Worker](../site/app/worker.js) and the [temporary Worker](../installer/worker/worker.js) while the latter remains in the repository. Do not restore the temporary Worker if an approved cutover has already removed it. The temporary Worker keeps its placeholder HTML; this proposal does not turn it into a second website.

Every root response adds both `Accept` and `User-Agent` to `Vary`, preserving other variation fields. Keep the [existing script caching and failure contract](../README.md#installer-routing): HTML does not require a release fetch, and an unavailable installer remains an uncached failure for script requests. `/install.sh` and every non-root asset path retain their existing behavior. Query parameters do not select a representation.

### Documentation and verification

- [Installation contract](../crates/pkg/docs/setup.md#installing-pkg): retain the standard curl command and identify `/install.sh` as the endpoint for custom HTTP clients or overridden user agents.
- [New-machine guide](../guides/new-machine.md#limits-and-edge-cases) and [site installation page](../site/app/content/docs/docs/install.md#installer-options): explain that explicit endpoint without changing the normal installation workflow.
- [Sharing preview reference](../README.md#sharing-previews): remove the Accept-only limitation once the new routing is implemented.
- Extend the [built site Worker tests](../site/tests/installer.test.ts) and [stub tests](../installer/worker/worker.test.js) with the request matrix above, HTML quality-zero and mixed-case cases, an unrelated user agent containing a curl token, and the new `Vary` fields. Crawler-shaped root requests must expose the site's existing metadata without fetching an installer. Keep the unavailable-release and explicit-script tests.

## Compatibility

The published curl command and standard wget requests keep working. A custom downloader, curl with a replaced user agent, or a client with no user agent may now receive HTML where it previously received a script. Those callers must use `https://gotpkg.dev/install.sh`. Package operations, installer options, installed files, and release artifacts do not change.

## Implications on adoption

The deployed Worker must be updated before public sharing benefits. Deployment remains separately authorized; proposal approval alone does not authorize it. No credentials, new infrastructure, or client version floor are added. Reverting routing restores the previous behavior without migrating stored data.

## Scope

- Root selection, cache variation, matching tests, and the affected references and guides.
- Consistent behavior in both Workers while they coexist.

### Out of scope

- OpenGraph artwork and tags, delivered in PR #5.
- Deploying the site, transferring the domain, or retiring the temporary Worker; those remain the separately approved cutover tracked as `pkg-8db`.
- Crawler authentication, bot allowlists, general Accept negotiation, preview-cache purges, and installer or CLI changes.

## Preview

- Artifact: the built Worker served locally with `mise run site:preview`, reachable through the existing private LAN preview.
- Exercise: open the website and inspect responses for the request matrix, including image retrieval and release-unavailable cases. Download installer responses for inspection only; do not execute them.
- Reason: this exercises the actual Worker and assets without a deployment. Rendering by external social platforms requires a separately authorized public deployment and is not claimed from local HTTP tests.
- Cleanup: stop the local preview when no longer needed. There is no new preview service or public environment to retain.

## Policies and decisions checked

No policy or decision records apply; both directories contain only their index. The [vision's one-command installation goal](../VISION.md#goals) is preserved. The conflicting Accept-only requirement is in the current README contract and is the explicit delta of this proposal; completed proposal 0002 is historical and remains unchanged.

## Future directions

None required for this change.

## Alternatives considered

- Recognize known sharing crawlers: preserves the installer default for unknown clients, but a new crawler or changed identifier can still receive the script. Mark selected the HTML default instead.
- Serve HTML for every root request and require `/install.sh` universally: simpler routing, but breaks the published curl command.
- Add metadata without changing routing: useful for HTML responses and docs, but leaves wildcard homepage crawlers unable to read it.

## Open questions

None.
