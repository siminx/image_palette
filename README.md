# image_palette

🎨 A Rust library for automatically extracting prominent color palettes from images.

Using octree algorithm, thanks for [color-extraction](https://github.com/xiong35/color-extraction).

# Installation

To use `image_palette` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
image_palette = "0.1.0"
```

## Usage

Here is a basic example that demonstrates how to extract the color palette and find the dominant colors.

```rust

fn main() {
  let colors = image_palette::load("test.jpg").unwrap();

  for item in colors {
    println!("{}:{}", item.color(), item.count());
  }
}
```
