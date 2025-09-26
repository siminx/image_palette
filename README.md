# image_palette

🎨 A Rust library for automatically extracting prominent color palettes from images.

Using octree algorithm, thanks for [color-extraction](https://github.com/xiong35/color-extraction).

# Installation

To use `image-palette` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
image-palette = "0.1"
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
