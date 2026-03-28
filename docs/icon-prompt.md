# App Icon Prompt

Use this prompt with an image generation AI (DALL-E, Midjourney, etc.) to create the readOut app icon.

## Prompt

```
Design a macOS app icon for "readOut", a real-time measurement dashboard app for multimeters and USB-C power meters.

The icon should convey: precision measurement, live data, electronics.

Visual elements to incorporate:
- A stylized digital multimeter display or voltage readout showing a value like "12.04"
- A subtle sine wave or signal line in teal/cyan color
- Clean, minimal, modern aesthetic matching macOS design language

Color palette:
- Primary: teal/cyan (#00C8AF) — the app's accent color
- Background: dark blue-gray (#0A0C10) — the app's dark theme base
- Signal line: bright blue (#3CAAFA) for multimeter, orange (#FFA03C) for USB-C

Style:
- Rounded square (macOS icon shape)
- Flat/semi-flat design with subtle depth
- No photorealism — clean vector-style illustration
- Should be recognizable at 16x16 and look great at 1024x1024
- Professional, technical, but not boring

Do NOT include: text labels, generic gear/settings icons, stock photo elements, gradients that look dated.
```

## Variations to try

1. **Readout focused**: Digital number display (monospace font showing voltage) with a small waveform underneath
2. **Signal focused**: Oscilloscope-style sine wave on dark background with teal glow
3. **Meter focused**: Simplified multimeter probe tips with a spark/signal between them
4. **Abstract**: Geometric representation of a measurement — a line chart peak with precise tick marks

## Technical requirements

- Export as 1024x1024 PNG (Apple will generate all smaller sizes)
- For macOS `.icns`: use `iconutil` to convert from `.iconset` directory
- Transparent background NOT recommended for macOS (use the dark bg)
