# Native framebuffer review

`native_capture` runs the real App with wgpu, native services and recovery. It
requests a framebuffer even while the viewport is occluded, writes a PNG and
closes after a three-second inspection interval. This developer example is
explicitly invoked; ordinary tests and desktop startup do not run it.

Use an empty temporary profile so review runs do not load user preferences,
recovery candidates or documents:

```sh
review_dir="$(mktemp -d)"
mkdir -p "$review_dir/config"
cat > "$review_dir/config/settings.json" <<'JSON'
{"schema":"paintcraft.settings/v2","experience":"classic","classic_variant":"v11","appearance":"dark"}
JSON
PAINTCRAFT_CONFIG_DIR="$review_dir/config" \
PAINTCRAFT_RECOVERY_DIR="$review_dir/recovery" \
cargo run -p paintcraft --example native_capture -- "$review_dir/client.png"
```

Change `appearance` to `light` for the other theme. The output directory must
exist. Missing arguments, failed encoding/writes and closing before readback
produce a nonzero exit. The PNG contains client pixels, without OS title/menu
bars. The example has its own executable identity; use the ordinary `paintcraft`
binary to qualify application-menu naming and normal window activation.

The explicit readback can force an otherwise occluded viewport to paint. It
therefore proves native framebuffer rendering, while an independent capture of
the ordinary desktop window is needed to qualify normal presentation. Neither
capture alone proves full Windows reference parity or interactive-device support.
