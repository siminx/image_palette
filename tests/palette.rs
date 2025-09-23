#[cfg(test)]
mod tests {

    #[test]
    fn test_add() {
        let (colors, width, height) = image_palette::load("test.jpg").unwrap();
        println!("total: {}", width * height);
        for color in colors {
            println!("{}: {}", color.rgb().to_hex(), color.count());
        }
    }
}
