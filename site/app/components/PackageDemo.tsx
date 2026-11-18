import { useState, useSyncExternalStore } from "react";

import CopyCommand from "./CopyCommand.tsx";

const packages = [
    { name: "wget", kind: "Homebrew formula", id: "brew:wget", mark: "↗" },
    { name: "IINA", kind: "Mac app", id: "brew-cask:iina", mark: "▷" },
    { name: "ripgrep", kind: "Developer tool", id: "ripgrep", mark: "⌕" },
    { name: "Prettier", kind: "npm package", id: "npm:prettier", mark: "≋" },
];

const noSubscription = () => () => {};

export default function PackageDemo() {
    const hydrated = useSyncExternalStore(
        noSubscription,
        () => true,
        () => false,
    );
    const [selected, setSelected] = useState(packages[0]!);
    const command = `pkg install ${selected.id}`;

    return (
        <div className="package-index">
            <fieldset
                aria-label="Choose an example package"
                className="package-choices"
                disabled={!hydrated}
            >
                {packages.map(item => (
                    <button
                        aria-pressed={selected.id === item.id}
                        className="package-choice"
                        key={item.id}
                        onClick={() => setSelected(item)}
                        type="button"
                    >
                        <span aria-hidden="true" className="package-mark">
                            {item.mark}
                        </span>
                        <span className="package-name">
                            {item.name}
                            <span>{item.kind}</span>
                        </span>
                        <span aria-hidden="true" className="package-check">
                            {selected.id === item.id ? "✓" : "+"}
                        </span>
                    </button>
                ))}
            </fieldset>
            <div className="demo-command">
                <div className="command-caption">
                    <span>One command. Packed for later.</span>
                    <span aria-hidden="true">↙</span>
                </div>
                <div className="command-line">
                    <span aria-hidden="true" className="prompt">
                        $
                    </span>
                    <code aria-live="polite">{command}</code>
                    <CopyCommand command={command} enabled={hydrated} key={command} />
                </div>
            </div>
            <p className="demo-note">Pick a package to see its command. Nothing runs here.</p>
        </div>
    );
}
