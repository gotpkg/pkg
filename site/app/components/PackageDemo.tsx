import { useState, useSyncExternalStore } from "react";

import CopyCommand from "./CopyCommand.tsx";
import Icon from "./icon/Icon.tsx";
import type { IconName } from "./icon/icons.ts";

const packages: { name: string; kind: string; id: string; icon: IconName }[] = [
    { name: "wget", kind: "Homebrew formula", id: "brew:wget", icon: "arrow-ne" },
    { name: "IINA", kind: "Mac app", id: "brew-cask:iina", icon: "playback-play" },
    { name: "ripgrep", kind: "Developer tool", id: "ripgrep", icon: "search" },
    { name: "Prettier", kind: "npm package", id: "npm:prettier", icon: "code" },
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
                        <Icon className="package-mark" name={item.icon} />
                        <span className="package-name">
                            {item.name}
                            <span>{item.kind}</span>
                        </span>
                        <Icon
                            className="package-check"
                            name={selected.id === item.id ? "checkmark" : "add"}
                        />
                    </button>
                ))}
            </fieldset>
            <div className="demo-command">
                <div className="command-caption">
                    <span>One command. Packed for later.</span>
                    <Icon name="arrow-sw" />
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
