import add from "../../assets/icons/Primaries-Add.svg?url";
import arrowNe from "../../assets/icons/Primaries-ArrowNE.svg?url";
import arrowRight from "../../assets/icons/Primaries-ArrowRight.svg?url";
import arrowSw from "../../assets/icons/Primaries-ArrowSW.svg?url";
import arrowUp from "../../assets/icons/Primaries-ArrowUp.svg?url";
import brightnessHigh from "../../assets/icons/Primaries-BrightnessHigh.svg?url";
import checkmark from "../../assets/icons/Primaries-Checkmark.svg?url";
import code from "../../assets/icons/Primaries-Code.svg?url";
import computer from "../../assets/icons/Primaries-Computer.svg?url";
import copy from "../../assets/icons/Primaries-Copy.svg?url";
import moon from "../../assets/icons/Primaries-Moon.svg?url";
import packageIcon from "../../assets/icons/Primaries-Package.svg?url";
import playbackPlay from "../../assets/icons/Primaries-PlaybackPlay.svg?url";
import search from "../../assets/icons/Primaries-Search.svg?url";

export const iconUrls = {
    add,
    "arrow-ne": arrowNe,
    "arrow-right": arrowRight,
    "arrow-sw": arrowSw,
    "arrow-up": arrowUp,
    "brightness-high": brightnessHigh,
    checkmark,
    code,
    computer,
    copy,
    moon,
    package: packageIcon,
    "playback-play": playbackPlay,
    search,
};

export type IconName = keyof typeof iconUrls;
