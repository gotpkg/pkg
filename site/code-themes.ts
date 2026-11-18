import type { StarlightExpressiveCodeOptions } from "@astrojs/starlight/expressive-code";

export const codeThemes = [
    {
        name: "pkg-dark",
        type: "dark",
        colors: {
            "editor.background": "#161619",
            "editor.foreground": "#f7f5ed",
            "editor.selectionBackground": "#fed90a35",
            "editorLineNumber.foreground": "#a3a096",
            "editorCursor.foreground": "#fed90a",
            "terminal.background": "#161619",
            "terminal.foreground": "#f7f5ed",
            "titleBar.activeBackground": "#232326",
            "titleBar.activeForeground": "#f7f5ed",
            "titleBar.border": "#41413e",
            focusBorder: "#fed90a",
            "button.background": "#fed90a",
            "button.foreground": "#161619",
        },
        tokenColors: [
            {
                scope: ["comment", "punctuation.definition.comment"],
                settings: { foreground: "#a3a096", fontStyle: "italic" },
            },
            {
                scope: ["keyword", "storage", "entity.name.function", "support.function"],
                settings: { foreground: "#fed90a" },
            },
            { scope: ["string", "markup.inline.raw"], settings: { foreground: "#eadf9a" } },
            {
                scope: ["constant", "variable.parameter", "entity.other.attribute-name"],
                settings: { foreground: "#e9bd74" },
            },
            {
                scope: ["variable", "entity.name.type", "support.type"],
                settings: { foreground: "#f7f5ed" },
            },
            {
                scope: ["punctuation", "meta.brace", "keyword.operator"],
                settings: { foreground: "#c5c2b8" },
            },
            { scope: "invalid", settings: { foreground: "#ffa28c", fontStyle: "underline" } },
        ],
    },
    {
        name: "pkg-light",
        type: "light",
        colors: {
            "editor.background": "#f0eee5",
            "editor.foreground": "#161619",
            "editor.selectionBackground": "#fed90a80",
            "editorLineNumber.foreground": "#69665b",
            "editorCursor.foreground": "#695700",
            "terminal.background": "#f0eee5",
            "terminal.foreground": "#161619",
            "titleBar.activeBackground": "#e8e5d9",
            "titleBar.activeForeground": "#161619",
            "titleBar.border": "#cbc6b7",
            focusBorder: "#695700",
            "button.background": "#fed90a",
            "button.foreground": "#161619",
        },
        tokenColors: [
            {
                scope: ["comment", "punctuation.definition.comment"],
                settings: { foreground: "#69665b", fontStyle: "italic" },
            },
            {
                scope: ["keyword", "storage", "entity.name.function", "support.function"],
                settings: { foreground: "#695700", fontStyle: "bold" },
            },
            { scope: ["string", "markup.inline.raw"], settings: { foreground: "#5d6023" } },
            {
                scope: ["constant", "variable.parameter", "entity.other.attribute-name"],
                settings: { foreground: "#805323" },
            },
            {
                scope: ["variable", "entity.name.type", "support.type"],
                settings: { foreground: "#161619" },
            },
            {
                scope: ["punctuation", "meta.brace", "keyword.operator"],
                settings: { foreground: "#585852" },
            },
            { scope: "invalid", settings: { foreground: "#a52c1f", fontStyle: "underline" } },
        ],
    },
] satisfies NonNullable<StarlightExpressiveCodeOptions["themes"]>;
