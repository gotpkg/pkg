import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const parser = fileURLToPath(new URL("../scripts/deployment-headers.awk", import.meta.url));
const cases = [
    {
        name: "uses the final success after Early Hints",
        headers:
            "HTTP/2 103\r\nLink: </style.css>; rel=preload\r\n\r\nHTTP/2 200\r\nX-Build: abc1234\r\n\r\n",
        expected: "200 abc1234\n",
    },
    {
        name: "does not mistake a CONNECT response for a successful page",
        headers:
            "HTTP/1.1 200 Connection established\r\n\r\nHTTP/2 404\r\nX-Build: abc1234\r\n\r\n",
        expected: "404 abc1234\n",
    },
    {
        name: "does not inherit a build stamp from an earlier response",
        headers:
            "HTTP/2 103\r\nX-Build: abc1234\r\n\r\nHTTP/2 200\r\nContent-Type: text/html\r\n\r\n",
        expected: "200 \n",
    },
    { name: "reports a transport failure when no headers arrive", headers: "", expected: "000 \n" },
];

for (const { name, headers, expected } of cases) {
    await test(name, () => {
        const parsed = spawnSync("awk", ["-f", parser], { input: headers, encoding: "utf8" });
        assert.equal(parsed.status, 0, parsed.stderr);
        assert.equal(parsed.stdout, expected);
    });
}
