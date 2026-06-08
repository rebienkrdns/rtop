---
name: Obsidian Terminal
colors:
  surface: '#12131b'
  surface-dim: '#12131b'
  surface-bright: '#383941'
  surface-container-lowest: '#0d0e15'
  surface-container-low: '#1a1b23'
  surface-container: '#1e1f27'
  surface-container-high: '#292932'
  surface-container-highest: '#33343d'
  on-surface: '#e3e1ed'
  on-surface-variant: '#bec9c6'
  inverse-surface: '#e3e1ed'
  inverse-on-surface: '#2f3038'
  outline: '#889391'
  outline-variant: '#3e4947'
  surface-tint: '#84d5cc'
  primary: '#92e3da'
  on-primary: '#003733'
  primary-container: '#76c7be'
  on-primary-container: '#00534d'
  inverse-primary: '#006a63'
  secondary: '#a5d566'
  on-secondary: '#203600'
  secondary-container: '#436c00'
  on-secondary-container: '#baec79'
  tertiary: '#ffc8bd'
  on-tertiary: '#5e160a'
  tertiary-container: '#ffa08e'
  on-tertiary-container: '#802e20'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#a0f1e8'
  primary-fixed-dim: '#84d5cc'
  on-primary-fixed: '#00201d'
  on-primary-fixed-variant: '#00504a'
  secondary-fixed: '#c0f27f'
  secondary-fixed-dim: '#a5d566'
  on-secondary-fixed: '#112000'
  on-secondary-fixed-variant: '#304f00'
  tertiary-fixed: '#ffdad3'
  tertiary-fixed-dim: '#ffb4a6'
  on-tertiary-fixed: '#3f0300'
  on-tertiary-fixed-variant: '#7d2c1e'
  background: '#12131b'
  on-background: '#e3e1ed'
  surface-variant: '#33343d'
typography:
  headline-lg:
    fontFamily: JetBrains Mono
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 32px
  headline-md:
    fontFamily: JetBrains Mono
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 24px
  body-md:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  body-sm:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
  label-caps:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '700'
    lineHeight: 16px
    letterSpacing: 0.05em
spacing:
  unit: 4px
  gutter: 16px
  margin: 12px
  container-padding: 8px
---

## Brand & Style

This design system is built for technical environments where information density and instant legibility are paramount. It draws heavily from **Brutalism** and **Modern Terminal Aesthetics**, prioritizing a dark, high-contrast environment that reduces eye strain during long-form monitoring and development tasks.

The brand personality is precise, efficient, and unapologetically technical. It evokes a sense of "under-the-hood" control, mimicking the utilitarian beauty of real-time system monitors. The visual language relies on sharp edges, monospaced structures, and a distinct "glowing" effect against a deep, near-black backdrop to guide the user's focus toward critical data changes.

## Colors

The palette is anchored by a deep **Obsidian** background, providing the necessary contrast for a multi-tonal status system.

- **Accent Cyan (#76C7BE):** Used for structural framing, headers, and active selection states. It acts as the primary "structural" color.
- **Vibrant Green (#A2D263):** Reserved for "Running," "Active," or "Healthy" states. It should be used for success indicators and positive data bars.
- **Soft Orange/Yellow (#EBC06D):** Communicates warnings, mid-level resource usage, or "Pending" states.
- **Coral Red (#E57B67):** High-priority alerts, errors, or critical resource exhaustion. 

The color application should mimic terminal "ANSI" styles, where colors are applied to specific data points or thin borders rather than large surface areas to maintain the dark-mode integrity.

## Typography

This system exclusively utilizes **JetBrains Mono** to ensure perfect character alignment and maximum readability for technical data. 

- **Alignment:** All numerical data must use tabular lining figures to ensure columns remain perfectly aligned in data grids.
- **Hierarchy:** Established primarily through color and weight rather than large jumps in scale. 
- **Caps:** Labels and headers often utilize all-caps with slight letter spacing to differentiate "Metadata" from "Active Data."

## Layout & Spacing

The layout follows a **Fixed Grid** philosophy inspired by terminal "panes." Content is organized into distinct, boxed modules that occupy specific percentages of the screen.

- **Grid:** A 12-column grid is used, but layout blocks are primarily defined by thin 1px borders.
- **Density:** High-density spacing is required. Internal padding within cards and lists should be kept to a minimum (8px) to maximize the amount of visible data.
- **Responsiveness:** On mobile, panes stack vertically. On desktop, the "Dashboard" view allows for side-by-side comparison of CPU, RAM, and Process lists.

## Elevation & Depth

Depth is not conveyed through shadows, but through **Bold Borders** and **Tonal Layering**.

- **Level 0 (Background):** The base Obsidian (#1A1B23) color.
- **Level 1 (Panes):** Defined by 1px solid borders using the Accent Cyan at 30-50% opacity. 
- **Level 2 (Active States):** Highlighting a row or a specific pane uses a subtle background tint of the accent color (10% opacity) and a full-opacity border.
- **Visual Dividers:** Use 1px lines rather than spacing to separate header sections from data content, maintaining the rigid, structured feel of a terminal.

## Shapes

The shape language is strictly **Sharp (0px)**. To maintain the "Terminal" and "Command Line" aesthetic, rounded corners are avoided entirely. 

Buttons, input fields, and container modules all feature crisp 90-degree angles. This reinforces the systematic, brutalist nature of the design and ensures that border alignments remain pixel-perfect across all screen resolutions.

## Components

- **Buttons:** Styled as text blocks with 1px borders. The "Active" state fills the background with Cyan and flips the text to the background color (Inverted).
- **Data Bars (Progress):** Backgrounds should be a dark neutral; the "fill" color represents the state (Green for low, Yellow for medium, Red for high usage).
- **Lists/Tables:** Use alternating row highlights (subtle gray) or no dividers at all, relying on alignment. The "Selected" row should have a Cyan background with dark text.
- **Chips/Status Tags:** Simple text strings wrapped in brackets, e.g., `[ RUNNING ]`, color-coded based on the status palette.
- **Input Fields:** 1px border on all sides or a bottom-border only. Include a "block" cursor character (`█`) in active states to mimic a CLI prompt.
- **Header Tabs:** Use solid blocks of color for the active tab to create a clear "Folder Tab" visual without using rounded corners.