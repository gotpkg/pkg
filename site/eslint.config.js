import astro from "eslint-plugin-astro";
import perfectionist from "eslint-plugin-perfectionist";
import tseslint from "typescript-eslint";

// ESLint lints `.astro` templates only. Oxlint (.oxlintrc.jsonc) lints everything else, including
// an `.astro` file's frontmatter and scripts.
export default [
    {
        ignores: ["dist/", ".astro/", ".wrangler/", "test-results/", "playwright-report/"],
    },
    // The flat-config array; the plain `recommended` key is the legacy format.
    ...astro.configs["flat/recommended"],
    {
        files: ["**/*.astro"],
        plugins: { perfectionist },
        // Frontmatter is TypeScript.
        languageOptions: { parserOptions: { parser: tseslint.parser } },
        rules: { "perfectionist/sort-jsx-props": "warn" },
    },
];
