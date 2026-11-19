export const siteUrl = "https://gotpkg.dev";

const imageUrl = new URL("/og-image.png", siteUrl).href;
const imageAlt = "The pkg logo in white with a yellow underline on a charcoal background.";

export const sharingHead: { tag: "meta"; attrs: Record<string, string> }[] = [
    { tag: "meta", attrs: { property: "og:site_name", content: "pkg" } },
    { tag: "meta", attrs: { property: "og:image", content: imageUrl } },
    { tag: "meta", attrs: { property: "og:image:type", content: "image/png" } },
    { tag: "meta", attrs: { property: "og:image:width", content: "2400" } },
    { tag: "meta", attrs: { property: "og:image:height", content: "1260" } },
    { tag: "meta", attrs: { property: "og:image:alt", content: imageAlt } },
    { tag: "meta", attrs: { name: "twitter:card", content: "summary_large_image" } },
    { tag: "meta", attrs: { name: "twitter:image", content: imageUrl } },
    { tag: "meta", attrs: { name: "twitter:image:alt", content: imageAlt } },
];
