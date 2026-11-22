import { iconUrls, type IconName } from "./icons.ts";

export default function Icon({ className, name }: { className?: string; name: IconName }) {
    return (
        <span
            aria-hidden="true"
            className={className ? `icon ${className}` : "icon"}
            style={{ maskImage: `url("${iconUrls[name]}")` }}
        />
    );
}
