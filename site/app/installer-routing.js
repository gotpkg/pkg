export const ROOT_VARIATION = "Accept, User-Agent";

// Only the first product counts: `Mozilla/5.0 curl/8.7.1` is not curl.
const DOWNLOADER = /^(?:curl|wget)\/[\w!#$%&'*+.^`|~-]+(?:\s|$)/i;

// RFC 9110's qvalue: zero to one, with at most three decimals.
const QUALITY = /^(?:0(?:\.\d{0,3})?|1(?:\.0{0,3})?)$/;

export function wantsInstaller(headers) {
    if (acceptsHtml(headers.get("Accept") ?? "")) return false;
    return DOWNLOADER.test(headers.get("User-Agent") ?? "");
}

/** Whether `accept` lists `text/html` itself, not a wildcard, with a positive quality. */
function acceptsHtml(accept) {
    for (const range of listItems(accept, ",")) {
        const parts = listItems(range, ";");
        if (parts[0].trim().toLowerCase() === "text/html" && positiveQuality(parts)) return true;
    }
    return false;
}

function positiveQuality(parts) {
    for (let index = 1; index < parts.length; index++) {
        const text = parts[index];
        const equals = text.indexOf("=");
        const name = (equals < 0 ? text : text.slice(0, equals)).trim().toLowerCase();
        if (name !== "q") continue;
        const quality = equals < 0 ? "" : text.slice(equals + 1).trim();
        if (!QUALITY.test(quality) || Number(quality) <= 0) return false;
    }
    return true;
}

function listItems(text, separator) {
    const items = [];
    let start = 0;
    let quoted = false;
    for (let index = 0; index < text.length; index++) {
        const character = text[index];
        if (quoted && character === "\\") {
            index++; // A quoted pair's escaped character is never a quote or separator.
        } else if (character === '"') {
            quoted = !quoted;
        } else if (character === separator && !quoted) {
            items.push(text.slice(start, index));
            start = index + 1;
        }
    }
    items.push(text.slice(start));
    return items;
}
