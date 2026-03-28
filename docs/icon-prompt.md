# App Icon Prompt

Use this prompt with an image generation AI (DALL-E, Midjourney, etc.) to create the readOut app icon.

## Prompt

```
Design a macOS app icon for "readOut", a real-time measurement dashboard for multimeters and USB-C power meters.

The icon is a single clean sine wave on a dark background. Nothing else. No text, no numbers, no UI elements, no grid lines.

The sine wave:
- One smooth, continuous sine wave spanning the full width of the icon
- Teal/cyan color (#00C8AF) with a soft glow or bloom effect
- The line should feel alive — like a live signal on an oscilloscope
- Medium thickness, confident stroke

Background:
- Deep dark blue-gray (#0A0C10 to #10131A), subtle gradient allowed
- No grid, no axes, no labels — just darkness and the wave

Style:
- Extremely minimal — the wave IS the icon
- Rounded square (macOS icon shape)
- The glow gives it depth without being skeuomorphic
- Should read clearly at 16x16 (just a glowing curve) and look stunning at 1024x1024
- Think: if Apple made an oscilloscope app

Do NOT include: text, numbers, grid lines, axes, UI chrome, probe tips, realistic elements, multiple waves, gradients that look dated.
```

## Variations to try

1. **Single sine**: One perfect sine wave, centered, teal glow
2. **Dual signal**: Teal sine wave (top) with a subtle orange (#FFA03C) wave below — representing both meters
3. **Pulse**: Instead of a sine, a single sharp pulse/peak — like a voltage spike captured in time
4. **Fading trail**: The wave fades from bright on the right to dim on the left — suggesting live, moving data

## Technical requirements

- Export as 1024x1024 PNG (Apple will generate all smaller sizes)
- For macOS `.icns`: use `iconutil` to convert from `.iconset` directory
- Transparent background NOT recommended for macOS (use the dark bg)
