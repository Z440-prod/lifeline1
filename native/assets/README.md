# App icons & splash screens

`icon.svg` is the single source. Generate every platform size with one command
using `@capacitor/assets` (already a dev dependency in `native/package.json`):

```bash
cd native
# @capacitor/assets reads assets/icon.(svg|png) and assets/splash.(svg|png).
npx @capacitor/assets generate --iconBackgroundColor '#0e1116' \
                               --splashBackgroundColor '#000000' \
                               --iconBackgroundColorDark '#0e1116' \
                               --splashBackgroundColorDark '#000000'
```

This writes all iOS `AppIcon` sizes + `Splash` and all Android `mipmap`/adaptive
icons into the generated `ios/` and `android/` projects. Re-run after any icon
change. For best results provide a 1024×1024 `icon.png` too (some generators
prefer PNG source); export it from `icon.svg` once.

The web/PWA icon lives separately in `web/manifest.webmanifest` (an inline SVG),
so the browser install and the store icons stay in sync by design.
