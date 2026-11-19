import { useId, useState } from "react";

import Icon from "./icon/Icon.tsx";

export default function CopyCommand({ command, enabled }: { command: string; enabled: boolean }) {
    const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
    const statusId = useId();

    async function copy() {
        try {
            await navigator.clipboard.writeText(command);
            setStatus("copied");
        } catch {
            setStatus("failed");
        }
    }

    return (
        <div className="copy-control">
            <button
                aria-describedby={statusId}
                aria-label="Copy command"
                className="copy-button"
                disabled={!enabled}
                onClick={copy}
                type="button"
            >
                <Icon name="copy" />
                {status === "copied" && <span className="copy-confirmation">Copied!</span>}
            </button>
            <output className={status === "failed" ? "copy-error" : "sr-only"} id={statusId}>
                {status === "copied" && "Command copied to clipboard."}
                {status === "failed" && "Couldn’t copy. Select and copy the command instead."}
            </output>
        </div>
    );
}
