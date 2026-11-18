---
name: pkg
description: A friendly packing list for your whole machine
colors:
    yellow: "#fed90a"
    ink: "#161619"
    paper: "#f7f5ed"
    white: "#ffffff"
typography:
    display:
        fontFamily: "Barlow, Arial, sans-serif"
        fontWeight: 800
        lineHeight: 0.95
        letterSpacing: "-0.035em"
    body:
        fontFamily: "Barlow, Arial, sans-serif"
        fontWeight: 400
        lineHeight: 1.5
    code:
        fontFamily: "JetBrains Mono, monospace"
        lineHeight: 1.6
rounded:
    control: "6px"
    panel: "12px"
spacing:
    small: "8px"
    medium: "16px"
    large: "32px"
---

# Design System: pkg

## Overview

The supplied identity becomes a software packing list: oversized, tightly set lettering; a yellow parcel label; selectable package names; and dark command strips. A developer opening this site during a workday gets a light, high-contrast reading surface. The supplied school poster adds campaign humor without becoming product evidence.

## Colors

Yellow owns the package demonstration and the closing installation section. Ink carries type, terminal examples, and the poster section. Paper supports longer reading. The logo's actual yellow takes precedence over the approximate yellow in the reference image.

## Typography

Barlow's heavy italic display echoes the supplied forward-leaning logo. Upright Barlow supports prose and navigation. JetBrains Mono is reserved for executable commands, package identifiers, and configuration.

## Layout

Wide screens pair the introductory offer with a package index. Sections alternate between the wide index, spacious explanatory prose, and the full campaign image. Below 760px, the hero and campaign stack; controls wrap rather than disappear. Documentation retains Starlight's familiar sidebar and reading column.

## Elevation & Depth

The demonstration's label has a restrained offset shadow and a small rotation. Everything else uses flat color fields. There are no glowing edges or decorative gradients.

## Shapes

Rounded rectangles belong to actions and the package label. Broad rectangular color fields organize the page. The supplied handwritten underline remains part of the logo, not a repeated decoration.

## Components

Package choices are real buttons with an announced selected state. Choosing one changes an illustrative command without installing anything. Copy buttons announce success or actionable failure. Navigation is visible at mobile sizes. Starlight supplies search, theme selection, table of contents, and mobile navigation.

Code blocks use the `pkg-dark` and `pkg-light` Expressive Code themes in `site/code-themes.ts`. Charcoal and paper replace the default editor surfaces. Yellow commands, wheat strings, amber constants, and warm gray comments keep syntax within the brand; the light theme uses darker mustard and olive equivalents for readable contrast. Theme selection follows Starlight's light/dark control.

## Do's and Don'ts

- Do use real commands and clearly describe examples.
- Do keep all content visible without animation and honor reduced motion.
- Do preserve Starlight's reading and accessibility affordances.
- Don't turn the campaign poster's joke commands into instructions.
- Don't publish release availability, adoption figures, or installation endpoints without evidence.
