#[cfg(test)]
#[cfg(feature = "lab")]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    use image_palette::{load_by_image_with_options, PaletteOptions};

    /// 生成纯色 RGBA 图
    fn solid_image(w: u32, h: u32, rgb: [u8; 3]) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    /// 生成水平条带图：每个颜色占满一条横向带
    fn banded_image(w: u32, colors: &[[u8; 3]]) -> DynamicImage {
        let band_h = 1u32;
        let h = colors.len() as u32 * band_h;
        let mut img = RgbaImage::new(w, h);
        for (i, color) in colors.iter().enumerate() {
            for x in 0..w {
                img.put_pixel(x, i as u32, Rgba([color[0], color[1], color[2], 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn pure_color_collapses_to_one() {
        let img = solid_image(256, 256, [180, 180, 180]);
        let (colors, _w, _h) =
            load_by_image_with_options(&img, &PaletteOptions::default()).unwrap();
        assert_eq!(colors.len(), 1, "纯色图应只产出 1 个主色");
        let rgb = colors[0].rgb();
        assert!(
            (rgb.r as i16 - 180).abs() <= 2
                && (rgb.g as i16 - 180).abs() <= 2
                && (rgb.b as i16 - 180).abs() <= 2,
            "纯色主色应接近 #B4B4B4，实际 {}",
            rgb.to_hex()
        );
        let ratio = colors[0].count() as f32 / (256 * 256) as f32 * 100.0;
        assert!(
            ratio > 99.0,
            "纯色主色占比应接近 100%，实际 {ratio:.2}%"
        );
    }

    #[test]
    fn two_distinct_colors_not_merged() {
        // 红蓝各占一半，Lab 距离远，不应被合并
        let img = banded_image(256, &[[255, 0, 0], [0, 0, 255]]);
        let (colors, _w, _h) =
            load_by_image_with_options(&img, &PaletteOptions::default()).unwrap();
        assert_eq!(colors.len(), 2, "红蓝两色不应被合并");
        let mut has_red = false;
        let mut has_blue = false;
        for color in &colors {
            let rgb = color.rgb();
            if rgb.r > 200 && rgb.b < 50 {
                has_red = true;
            }
            if rgb.b > 200 && rgb.r < 50 {
                has_blue = true;
            }
            assert_ne!(rgb.to_hex(), "#000000", "主色不应被平均成黑色");
        }
        assert!(has_red && has_blue, "应同时保留红蓝，实际 {:?}", 
            colors.iter().map(|c| c.rgb().to_hex()).collect::<Vec<_>>());
    }

    #[test]
    fn close_grays_merge_into_few() {
        // 多个相近灰阶，ΔE 互不超过 8 的应被合并
        let grays: Vec<[u8; 3]> = (200..=215u8).map(|v| [v, v, v]).collect();
        let img = banded_image(256, &grays);

        // 不合并：基线
        let no_merge = PaletteOptions::default().with_merge_delta_e(0.0);
        let (colors_no_merge, _, _) = load_by_image_with_options(&img, &no_merge).unwrap();

        // 合并
        let (colors_merged, _, _) =
            load_by_image_with_options(&img, &PaletteOptions::default()).unwrap();

        assert!(
            colors_merged.len() < colors_no_merge.len(),
            "合并后主色数 {} 应少于不合并 {}",
            colors_merged.len(),
            colors_no_merge.len()
        );
        assert!(
            colors_merged.len() <= 3,
            "相近灰合并后应 ≤3 个，实际 {}",
            colors_merged.len()
        );
        assert!(
            colors_merged.len() >= 1,
            "合并后至少应保留 1 个主色"
        );
        // 回归：加权平均写错会把浅灰全部除成 #000000
        for color in &colors_merged {
            let rgb = color.rgb();
            assert!(
                rgb.r > 150 && rgb.g > 150 && rgb.b > 150,
                "相近灰合并后不应变黑，实际 {}",
                rgb.to_hex()
            );
        }
    }

    #[test]
    fn delta_e_zero_preserves_old_behavior() {
        // merge_delta_e = 0 时，与旧 load_by_image_with_maxcolor 行为一致：不合并
        let grays: Vec<[u8; 3]> = (200..=215u8).map(|v| [v, v, v]).collect();
        let img = banded_image(256, &grays);

        let opts = PaletteOptions::default().with_merge_delta_e(0.0);
        let (colors, _, _) = load_by_image_with_options(&img, &opts).unwrap();

        // 旧 API 等价调用
        let (colors_old, _, _) =
            image_palette::load_by_image_with_maxcolor(&img, opts.extract_max).unwrap();

        assert_eq!(
            colors.len(),
            colors_old.len(),
            "merge_delta_e=0 应等价于旧 API"
        );
    }

    #[test]
    fn colored_paper_sample_not_all_black() {
        let path = r"C:\Users\xusimin\Pictures\Saved Pictures\colored_paper2.png";
        let img = match image::open(path) {
            Ok(img) => img,
            Err(_) => {
                eprintln!("跳过不存在样本: {path}");
                return;
            }
        };
        let (colors, _, _) =
            load_by_image_with_options(&img, &PaletteOptions::default()).unwrap();
        assert!(!colors.is_empty(), "纸纹图应至少产出 1 个主色");
        assert!(
            (2..=4).contains(&colors.len()),
            "纸纹图主色数应在 2–4，实际 {}：{:?}",
            colors.len(),
            colors.iter().map(|c| c.rgb().to_hex()).collect::<Vec<_>>()
        );
        for color in &colors {
            assert_ne!(
                color.rgb().to_hex(),
                "#000000",
                "纸纹图主色不应为 #000000，实际 {:?}",
                colors.iter().map(|c| c.rgb().to_hex()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn same_hue_lightness_gradient_collapses() {
        // 同色相（棕/米）的明度梯度：从浅米到深棕，Lab 主要在 L 轴变化
        let browns: Vec<[u8; 3]> = [
            [230, 200, 170], [215, 185, 155], [200, 170, 140], [185, 155, 125],
            [170, 140, 110], [150, 120, 90], [130, 100, 70], [110, 80, 50],
        ]
        .to_vec();
        let img = banded_image(256, &browns);

        let no_merge = PaletteOptions::default().with_merge_delta_e(0.0);
        let (colors_no_merge, _, _) = load_by_image_with_options(&img, &no_merge).unwrap();
        let (colors_merged, _, _) =
            load_by_image_with_options(&img, &PaletteOptions::default()).unwrap();

        assert!(
            colors_merged.len() < colors_no_merge.len(),
            "同色相渐变合并后 {} 应少于不合并 {}",
            colors_merged.len(),
            colors_no_merge.len()
        );
        assert!(
            (2..=4).contains(&colors_merged.len()),
            "同色相渐变应合并到 2–4 个，实际 {}：{:?}",
            colors_merged.len(),
            colors_merged.iter().map(|c| c.rgb().to_hex()).collect::<Vec<_>>()
        );
        for color in &colors_merged {
            assert_ne!(color.rgb().to_hex(), "#000000", "同色相渐变不应变黑");
        }
    }
}
