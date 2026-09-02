# image_palette

🎨 A Rust library for automatically extracting prominent color palettes from images.

Using octree algorithm, thanks for [color-extraction](https://github.com/xiong35/color-extraction).

# Installation

To use `image-palette` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
image-palette = "0.3"
```

To enable similar-color merging (recommended), enable the `lab` feature so that
distances are measured in the CIELAB color space, which matches human perception
much better than RGB:

```toml
[dependencies]
image-palette = { version = "0.3", features = ["lab"] }
```

## Usage

Here is a basic example that demonstrates how to extract the color palette and find the dominant colors.

```rust
fn main() {
  let (colors, width, height) = image_palette::load("test.jpg").unwrap();
  println!("total: {}", width * height);
  for color in colors {
    println!("{}:{}", color.rgb().to_hex(), color.count());
  }
}
```

## Advanced Options

Since v0.3.0, `load_by_image_with_options` gives you full control over extraction,
similar-color merging, ratio filtering and output size via [`PaletteOptions`].

### `PaletteOptions` fields

| field             | default | description                                                                 |
| ----------------- | ------- | --------------------------------------------------------------------------- |
| `extract_max`     | `16`    | OctTree internal bin count. Larger values distinguish closer colors.       |
| `output_max`      | `8`     | Maximum number of colors returned.                                         |
| `merge_delta_e`   | `10.0`  | CIELAB ΔE merge threshold. `0.0` disables merging. Requires the `lab` feature. |
| `min_ratio`       | `0.01`  | Minimum pixel ratio (in percent) relative to **visible** (non-transparent) pixels. Colors below this are dropped. |

### Transparency

Pixels with `alpha == 0` are excluded from extraction. Semi-transparent pixels
(`alpha > 0`) are included using their RGB values. This prevents transparent
backgrounds (common in SVG/PNG) from being counted as black. `Record.count`
values sum to the number of visible pixels only; returned `width`/`height`
remain the full image dimensions. Fully transparent images yield an empty palette.

### Default usage

`PaletteOptions::default()` extracts up to 16 colors with the octree, merges
similar colors via agglomerative clustering, drops colors below `0.01%` of
visible pixels, and returns at most 8 colors. Images with a flat or monotonous
palette will naturally yield fewer than 8 colors.

Merging is **agglomerative clustering with centroid linkage**: every octree
color starts as its own cluster, and the two closest clusters are repeatedly
merged until the nearest pair is farther apart than `merge_delta_e`. The
distance is a lightness-discounted CIE76:

```text
ΔE' = sqrt((ΔL / 2)^2 + Δa^2 + Δb^2)
```

The lightness difference is halved, so a monotonous lightness gradient of the
same hue (e.g. a brown paper texture from light tan to dark brown) collapses
into a few representative colors (highlight / mid / shadow) instead of filling
all 8 slots with barely distinguishable shades. Colors that differ mainly on
the a/b axes — red vs blue, green vs orange — stay separate because their
distance is dominated by Δa/Δb, which is not discounted.

```rust
use image_palette::{load_by_image_with_options, PaletteOptions};

let image = image::open("test.jpg").unwrap();
let (colors, _w, _h) = load_by_image_with_options(&image, &PaletteOptions::default()).unwrap();
for color in &colors {
  println!("{}: {:.2}%", color.rgb().to_hex(), color.count());
}
```

### Customizing options

Options use a builder pattern, so you only override what you need:

```rust
use image_palette::PaletteOptions;

// Disable merging entirely (behave like the legacy API but still cap output)
let opts = PaletteOptions::default().with_merge_delta_e(0.0);

// Return up to 5 colors and only keep colors covering at least 1% of the image
let opts = PaletteOptions::default()
    .with_output_max(5)
    .with_min_ratio(1.0);

// Be more aggressive about merging similar colors
let opts = PaletteOptions::default().with_merge_delta_e(15.0);
```

### Backward compatibility

The legacy APIs `load`, `load_with_maxcolor` and `load_by_image_with_maxcolor`
keep working exactly as before — they are equivalent to calling
`load_by_image_with_options` with `merge_delta_e = 0.0`, `min_ratio = 0.0` and
`output_max = extract_max` (no merging, no ratio filtering, no truncation beyond
the requested color count). They do not require the `lab` feature.
