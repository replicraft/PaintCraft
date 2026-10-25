# Astro browser builds and Cloudflare Pages

PaintCraft Web uses Astro with `vite-plugin-wasm`. Astro/Vite bundle the Rust
module, browser scripts, hashed Wasm asset and static page. The editor remains in
Rust. No Cloudflare Functions or server-side runtime is required by the site.

## Cloudflare Pages settings

| Setting | Value |
| --- | --- |
| Framework preset | Astro |
| Root directory | `apps/paintcraft-web` |
| Build command | `npm run build` |
| Build output directory | `dist` |
| Environment variable | `NODE_VERSION=24` |

Cloudflare installs the locked npm dependencies. The npm `prebuild` step prepares
the Rust package, bootstraps rustup only if absent, selects the root pinned
`rust-toolchain.toml`, adds the wasm target and ensures the wasm-bindgen CLI
matches the locked ABI. Astro then builds the site and `postbuild` checks every
asset against the Pages upload limit. Python is not required for this pipeline.

Vite's Wasm plugin handles JavaScript/Wasm module integration; it does not replace
Cargo, the Rust compiler or wasm-bindgen. The npm lifecycle makes those preparation
steps automatic. See [Astro's Wasm imports](https://docs.astro.build/en/guides/imports/#webassembly)
and the [plugin documentation](https://github.com/Menci/vite-plugin-wasm).

Cloudflare [documents its build environment](https://developers.cloudflare.com/pages/configuration/build-image/)
and [20-minute build timeout](https://developers.cloudflare.com/pages/platform/limits/#builds).
Cold installation of Rust and the binding tool adds time. GitHub Actions caches
the binding tool and uploads a downloadable site artifact for direct deployment.

## Local builds and direct upload

From the repository root:

```sh
npm --prefix apps/paintcraft-web ci
npm --prefix apps/paintcraft-web run build
npx wrangler pages deploy apps/paintcraft-web/dist --project-name YOUR_PAGES_PROJECT
```

Replace the project name and authenticate Wrangler for your own account. For
CI publishing, configure account ID/token secrets in your deployment workflow.
Cloudflare supports [direct upload from CI](https://developers.cloudflare.com/pages/how-to/use-direct-upload-with-continuous-integration/).
The existing CI web job builds and uploads the `paintcraft-web` artifact; it does
not publish to a Cloudflare account. Upload only generated `dist` contents.

For development use `npm --prefix apps/paintcraft-web run dev` and open port 8088.
After Rust edits, rerun `npm --prefix apps/paintcraft-web run build:wasm`; Astro
handles browser-side script/page updates. `npm run preview` serves production
output. `CARGO_TARGET_DIR` is resolved through Cargo metadata.

## Size and font handling

The `web-release` Cargo profile retains optimization level 3, enables cross-crate
LTO and uses one codegen unit with stripped debug symbols. The binding step removes
the Wasm name section. Desktop release settings remain unchanged.

Font bytes are shared between the UI and document text renderer. Desktop embeds
them once. Browser Selawik and Devanagari remain embedded; the Japanese and Chinese
fonts are copied byte-for-byte from root `assets/fonts` into separate hashed static
assets with their licenses. The browser verifies sizes and SHA-256 checksums
before installing them and starting the editor. Missing/corrupt assets show a
startup error instead of missing glyphs. Initial startup needs these bundled
assets; this does not add a service worker or offline cache.

The postbuild check enforces Pages' [25 MiB limit per file](https://developers.cloudflare.com/pages/platform/limits/#file-size)
on actual uncompressed asset bytes. HTTP compression does not bypass this limit.
The previous 47,018,124-byte Wasm asset is replaced by a substantially smaller
module; all font files are also individually below the limit. The build prints
the exact largest asset size.

## Validation scope

The production Astro build, font checks and browser tests are executed locally.
CI repeats the production build on Linux. A local build and headless browser test
do not prove deployment to Cloudflare; account/project configuration, publishing
and the public site still require a real deployment.
