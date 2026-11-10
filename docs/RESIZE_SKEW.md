# Resize and skew

Resize and skew resamples actual editable pixels; canvas-size editing remains a separate operation that changes the canvas boundary without stretching pixels. The dialog stages its values until OK, and Cancel leaves the document unchanged. A changed source document or target invalidates the draft.

With no active selection, stretch/skew transforms every layer, including hidden and off-canvas content, and updates the canvas boundary. Layer identities, ordering, blend/opacity, metadata and editable filter parameters remain intact. Stored own-layer and filter masks, including disabled masks, transform with their layer. This resamples source pixels before the retained filters; it does not bake the filtered composite.

With an active selection, only selected pixels of the active layer transform. Unselected pixels remain; the old selected area clears using the frontend's existing ground color. Selection holes remain holes. The transformed selection is clipped to the current canvas, while transformed pixels outside that canvas remain stored in the layer. Empty and identity operations do not add history.

The first implementation uses nearest-neighbor pixel-center sampling. It stretches first, shears horizontally, then shears vertically, and expands the finite bounding rectangle. Negative angles are supported. The two shears are sequential, so using both does not introduce the singular simultaneous-shear matrix. The angle is converted once to a signed Q16 tangent; persisted batches carry those integer coefficients and use integer geometry on native and Wasm.

Admission, cancellation and publication use the ordinary atomic edit transaction. Destination sample visits share the existing 16,777,216 transform allowance across a batch; mask work, staged tiles and decoded residency have their existing independent limits. Browser canvas policy remains four megapixels. A request that exceeds a limit fails without publishing partial pixels or changing history. Stored coordinate Noise still requires the existing explicit bake before geometry changes.

Automation uses the additive `paintcraft.batch/stretch-skew-v1` schema (`paintcraft-cli schema batch-stretch-skew`), with the same exact sorted `required_filters` and `required_layer_features` inventories as the appearance envelope. Earlier published schema files remain unchanged and reject the new operations. `canvas.stretch_skew` requires `width`, `height`, and a two-integer `shear` array. `raster.stretch_skew_selection` additionally requires `layer_ref` and `clear`. Width and height describe the stretched size before skew; the resulting expanded boundary may be larger. Coefficients are bounded to ±3,754,555. CLI `apply --operations` uses the normal base-state, publication-epoch and atomic-output checks.

This is a Win95-inspired editing workflow, not a claim of bit-identical historical Windows resampling or complete historical Paint parity.
