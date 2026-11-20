import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";
import { defineCollection } from "astro:content";

// Starlight's pages: `app/content/docs/docs/install.md` is served at `/docs/install`.
export const collections = {
    docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
};
