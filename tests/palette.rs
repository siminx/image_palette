#[cfg(test)]
mod tests {

    #[test]
    fn test_add() {
        let (colors, width, height) = image_palette::load("test.jpg").unwrap();
        println!("total: {}", width * height);
        for item in colors {
            println!("{}: {}", item.color().to_hex(), item.count());
        }
    }
}
