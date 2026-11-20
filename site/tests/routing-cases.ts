type RequestCase = { name: string; headers: Record<string, string> };

export const htmlRequests: RequestCase[] = [
    { name: "missing headers", headers: {} },
    { name: "browser with wildcard", headers: { Accept: "*/*", "User-Agent": "Mozilla/5.0" } },
    { name: "Twitter preview", headers: { Accept: "*/*", "User-Agent": "Twitterbot/1.0" } },
    {
        name: "Messages preview",
        headers: {
            Accept: "*/*",
            "User-Agent": "Mozilla/5.0 facebookexternalhit/1.1 Facebot Twitterbot/1.0",
        },
    },
    {
        name: "unknown JSON client",
        headers: { Accept: "application/json", "User-Agent": "custom-client/1" },
    },
    {
        name: "curl mentioned later",
        headers: { Accept: "*/*", "User-Agent": "Mozilla/5.0 curl/8.7.1" },
    },
    { name: "curl without a version", headers: { Accept: "*/*", "User-Agent": "curl/" } },
    { name: "bare curl token", headers: { Accept: "*/*", "User-Agent": "curl" } },
    { name: "HTML override", headers: { Accept: "text/html", "User-Agent": "curl/8.7.1" } },
    {
        name: "mixed-case weighted HTML",
        headers: { Accept: "application/json, TEXT/HTML;Q=0.5", "User-Agent": "Wget/1.25.0" },
    },
    {
        name: "HTML with parameters",
        headers: { Accept: "text/html;charset=utf-8;q=1.000", "User-Agent": "curl/8.7.1" },
    },
    {
        name: "quoted parameters before positive quality",
        headers: { Accept: 'text/html;profile="a,b;c";q=0.5', "User-Agent": "curl/8.7.1" },
    },
];

export const installerRequests: RequestCase[] = [
    { name: "standard curl", headers: { Accept: "*/*", "User-Agent": "curl/8.7.1" } },
    { name: "wget without Accept", headers: { "User-Agent": "Wget/1.25.0" } },
    { name: "mixed-case curl product", headers: { "User-Agent": "CURL/8.7.1" } },
    {
        name: "wget with a platform comment",
        headers: { Accept: "text/*", "User-Agent": "wget/1.25.0 (linux-gnu)" },
    },
    ...["0", "0.000", "", "bogus", "1junk", "1.1", "-1", ".5", "0.1234"].map(quality => ({
        name: `HTML with nonpositive or malformed quality ${quality}`,
        headers: { Accept: `text/html;q=${quality}`, "User-Agent": "curl/8.7.1" },
    })),
    {
        name: "not an HTML media range",
        headers: { Accept: "application/text/html", "User-Agent": "curl/8.7.1" },
    },
    {
        name: "zero quality after a quoted comma",
        headers: { Accept: 'text/html;profile="a,b;c";q=0', "User-Agent": "curl/8.7.1" },
    },
    {
        name: "HTML text inside another media parameter",
        headers: {
            Accept: 'application/json;profile="x,text/html;q=1"',
            "User-Agent": "curl/8.7.1",
        },
    },
];
