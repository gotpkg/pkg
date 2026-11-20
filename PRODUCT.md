# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

People who want one place to install and manage their apps, system packages, and developer tools. The website introduces the CLI; its documentation supports daily use.

## Product Purpose

pkg installs software through mise's bootstrap packages and tools, records installations in the global mise configuration, and restores that configuration on another machine with `pkg bootstrap`.

## Positioning

The unified system package manager: one command vocabulary for apps, system packages, and developer tools. Position the product without centering any one operating system. The long-term goal is availability on every platform; do not present that goal as current support. pkg remains a human-first command-line front end for mise; JSON output and built-in discovery also serve agents and scripts.

The user also wants the copy to emphasize that pkg just works out of the box and its commands do what people expect. Support this with its shared command vocabulary and defaults; explain the installer, starting fresh with `pkg setup`, and restoring with `pkg bootstrap` accurately.

## Capabilities and Constraints

Current behavior is owned by README.md and crates/pkg/docs/{cli,setup,packages}.md. Unimplemented proposals are not product capabilities. Installation on macOS and Linux uses the release-owned script at gotpkg.dev; `pkg setup` and `pkg bootstrap` install mise when needed. Windows uses a release archive and supports the package commands, not the installer, setup, or bootstrap. Do not claim a release is available without checking it; installer routes return an uncached 502 when GitHub cannot provide the script.

The website uses the local content-website template's Astro, React/Redact, Tailwind and Cloudflare stack. Starlight provides documentation under /docs. Its production domain is gotpkg.dev. The site's Worker replaces the temporary installer Worker only after an explicitly approved deployment and verification.

## Brand Commitments

Use the supplied pkg vector logos in assets. The supplied identity reference establishes bold rounded italic lettering, charcoal, yellow, and an off-white supporting surface. The user grants creative freedom for the site and permits use of the middle-school “Got pkg?” poster.

## Evidence on Hand

The CLI README and generated command reference describe supported commands. assets/pkg-logo-light-mode.svg and assets/pkg-logo-dark-mode.svg are supplied identity assets. The source campaign image is got-pkg.png in the user's supplied logo folder. There are no supplied testimonials, adoption figures, performance benchmarks, or customer claims.

The supplied moving-box artwork appears as a decorative image beside the final installation action, optimized at `site/app/assets/moving-box.webp`.
