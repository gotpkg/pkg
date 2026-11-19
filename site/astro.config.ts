import cloudflare from "@astrojs/cloudflare";
import react from "@astrojs/react";
import starlight from "@astrojs/starlight";
import tailwindcss from "@tailwindcss/vite";
import { redact } from "@tanstack/redact/vite";
import { defineConfig } from "astro/config";

import { codeThemes } from "./code-themes.ts";

export default defineConfig({
    site: "https://gotpkg.dev",
    srcDir: "./app",
    // Keep the installer Worker in the output even though every page is prerendered.
    output: "server",
    // Canonical URLs and every internal link use the unslashed form; `assets.html_handling` in
    // wrangler.jsonc serves prerendered pages there without a redirect.
    trailingSlash: "never",
    // Astro 7's default, `'jsx'`, also collapses whitespace between inline elements (`</a> <a>`
    // loses its space). `true` keeps the pre-7 rendering of prose and inline markup.
    compressHTML: true,
    // Pages are prerendered. `compile` optimizes `astro:assets` images at build time, so the
    // Worker never transforms images and needs no Images binding.
    adapter: cloudflare({ imageService: "compile" }),
    integrations: [
        react(),
        // Starlight renders the Markdown in app/content/docs/ (its `docs/` folder puts every page
        // under /docs) and adds MDX, Expressive Code and the Pagefind search index itself.
        starlight({
            title: "pkg",
            logo: {
                light: "./app/assets/pkg-logo-light-mode.svg",
                dark: "./app/assets/pkg-logo-dark-mode.svg",
                // The logo is the word "pkg"; the title stays as its alternative text.
                replacesTitle: true,
            },
            social: [{ icon: "github", label: "GitHub", href: "https://github.com/gotpkg/pkg" }],
            sidebar: [
                "docs",
                "docs/install",
                "docs/setup",
                "docs/packages",
                "docs/configuration",
                "docs/automation",
                "docs/reference",
            ],
            customCss: ["./app/styles/docs.css"],
            expressiveCode: {
                themes: codeThemes,
                styleOverrides: {
                    borderRadius: "8px",
                    codeFontFamily: "var(--sl-font-mono)",
                    uiFontFamily: "var(--sl-font)",
                },
            },
        }),
    ],
    // Without this the adapter declares a `SESSION` KV binding that Wrangler provisions on
    // deploy. Enable sessions only with a route that uses them.
    session: false,
    vite: {
        plugins: [
            // Redact replaces React and React DOM in the browser and in server rendering, so
            // islands hydrate with the same `useId()` values the server rendered. The `nano`
            // preset stubs every optional feature; the islands use only hooks, so hydration and
            // `memo` (for @astrojs/react's static-HTML wrapper) are all they turn back on. An
            // island that needs context, Suspense, portals or another stubbed feature enables it
            // here.
            redact({ preset: "nano", features: { hydration: true, memo: true } }),
            tailwindcss(),
        ],
    },
});
