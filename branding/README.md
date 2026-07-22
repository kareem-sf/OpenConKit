# OpenConKit brand assets

Canonical brand sources live here at the repo root (`branding/`). The desktop
UI imports them directly (Vite resolves outside the app root), so there is a
single source of truth.

- `logo.svg` - full-color logo mark (open frame + modular kit blocks on a construction grid).
- `logo-mono.svg` - monochrome variant using `currentColor`; use on colored or busy surfaces.
- `icon.svg` - square app-icon source (dark tile, amber mark).

## Usage rules

- Do not recolor, stretch, or redraw the mark; use the mono variant when the
  full-color palette does not fit.
- Minimum clear space: the height of one kit block on all sides.
- The accent palette (`--color-brand-*`) is defined in `packages/ui/src/tokens.css`.

## Generating the Tauri icon set

```sh
pnpm icons:generate
```

This rasterizes `icon.svg` to a 1024x1024 PNG (`branding/icon-1024.png`) and
runs `@tauri-apps/cli icon` to emit the icon set into
`crates/openconkit-desktop/icons/` (referenced by `tauri.conf.json`).
