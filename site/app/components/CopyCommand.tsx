import { useId, useState } from "react";

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
                <svg
                    aria-hidden="true"
                    fill="none"
                    height="18"
                    stroke="currentColor"
                    strokeWidth="1.7"
                    viewBox="0 0 24 24"
                    width="18"
                >
                    <rect height="13" rx="2" width="12" x="8" y="8" />
                    <path d="M15 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h3" />
                </svg>
                {status === "copied" && <span className="copy-confirmation">Copied!</span>}
            </button>
            <output className={status === "failed" ? "copy-error" : "sr-only"} id={statusId}>
                {status === "copied" && "Command copied to clipboard."}
                {status === "failed" && "Couldn’t copy. Select and copy the command instead."}
            </output>
        </div>
    );
}
