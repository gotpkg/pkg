---
name: pkg
description: A friendly packing list for your whole machine
colors:
    yellow: "#fed90a"
    ink: "#161619"
    paper: "#f7f5ed"
    white: "#ffffff"
    dark-bg: "#161619"
    dark-muted: "#b8b8b2"
    dark-border: "#3a3a3d"
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

The supplied identity becomes a software packing list: oversized, tightly set lettering; a yellow parcel label; selectable package names; and dark command strips. The landing page follows the visitor's system color preference. The supplied school poster adds campaign humor without becoming product evidence.

## Colors

Yellow owns the package demonstration and the closing installation section in both themes. Light mode uses paper with ink text; dark mode uses charcoal with paper text and muted warm gray supporting copy. Yellow panels retain dark text and use the supplied monochrome logo. Code examples retain light text on dark surfaces. The header and footer switch to the supplied reverse logo in dark mode. The logo's actual yellow takes precedence over the approximate yellow in the reference image.

## Typography

Barlow's heavy italic display echoes the supplied forward-leaning logo. Upright Barlow supports prose and navigation. JetBrains Mono is reserved for executable commands, package identifiers, and configuration.

Short labels use title case in source and the `uppercase` CSS class for their all-caps presentation.

## Layout

Wide screens pair the introductory offer with a package index. Sections alternate between the wide index, spacious explanatory prose, and the full campaign image. Below 760px, the hero and campaign stack; controls wrap rather than disappear. Documentation retains Starlight's familiar sidebar and reading column.

## Elevation & Depth

The demonstration's label has a restrained offset shadow and a small rotation. Everything else uses flat color fields. There are no glowing edges or decorative gradients.

## Shapes

Rounded rectangles belong to actions and the package label. Broad rectangular color fields organize the page. The supplied handwritten underline remains part of the logo, not a repeated decoration.

## Iconography

Interface icons come from the Primaries set: filled shapes on a 64-unit grid, drawn in the current text color so they follow both themes. Arrows mark links. In the package index each example has its own mark. The docs repaint Starlight's search, menu, theme, page navigation, heading link, and code copy icons with the same set; the GitHub mark in the docs header stays because it is a brand logo. The pkg logo and the supplied photographs are artwork, not icons. To add an icon, copy its Primaries file unchanged into `site/app/assets/icons/`; never redraw one or use a text character in its place.

## Components

Package choices are real buttons with an announced selected state. Choosing one changes an illustrative command without installing anything. Copy buttons announce success or actionable failure. Navigation is visible at mobile sizes. Starlight supplies search, theme selection, table of contents, and mobile navigation.

The landing page follows system theme changes through CSS, including without JavaScript or browser storage. Starlight's theme picker controls the documentation separately.

Code blocks use the `pkg-dark` and `pkg-light` Expressive Code themes in `site/code-themes.ts`. Charcoal and paper replace the default editor surfaces. Yellow commands, wheat strings, amber constants, and warm gray comments keep syntax within the brand; the light theme uses darker mustard and olive equivalents for readable contrast. Theme selection follows Starlight's light/dark control.

## Do's and Don'ts

- Do use real commands and clearly describe examples.
- Do keep all content visible without animation and honor reduced motion.
- Do preserve Starlight's reading and accessibility affordances.
- Don't turn the campaign poster's joke commands into instructions.
- Don't publish release availability, adoption figures, or installation endpoints without evidence.
