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
        assert!(ratio > 99.0, "纯色主色占比应接近 100%，实际 {ratio:.2}%");
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
        assert!(
            has_red && has_blue,
            "应同时保留红蓝，实际 {:?}",
            colors.iter().map(|c| c.rgb().to_hex()).collect::<Vec<_>>()
        );
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
        assert!(colors_merged.len() >= 1, "合并后至少应保留 1 个主色");
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
    fn same_hue_lightness_gradient_collapses() {
        // 同色相（棕/米）的明度梯度：从浅米到深棕，Lab 主要在 L 轴变化
        let browns: Vec<[u8; 3]> = [
            [230, 200, 170],
            [215, 185, 155],
            [200, 170, 140],
            [185, 155, 125],
            [170, 140, 110],
            [150, 120, 90],
            [130, 100, 70],
            [110, 80, 50],
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
            colors_merged
                .iter()
                .map(|c| c.rgb().to_hex())
                .collect::<Vec<_>>()
        );
        for color in &colors_merged {
            assert_ne!(color.rgb().to_hex(), "#000000", "同色相渐变不应变黑");
        }
    }
}

#[cfg(test)]
mod sampled_tests {
    use image::{DynamicImage, ImageBuffer, Rgb, RgbImage, Rgba, RgbaImage};

    use image_palette::{
        load_by_image_sampled_with_options, load_by_image_with_options, PaletteOptions, Record,
    };

    /// 采样测试关闭后处理，便于直接核对 OctTree 的面积权重。
    fn unmerged_options() -> PaletteOptions {
        PaletteOptions::default()
            .with_extract_max(255)
            .with_output_max(255)
            .with_merge_delta_e(0.0)
            .with_min_ratio(0.0)
    }

    /// 将记录转成与 HashMap 遍历顺序无关的可比较形式。
    fn normalized(records: &[Record]) -> Vec<(u8, u8, u8, u32)> {
        let mut values = records
            .iter()
            .map(|record| {
                let rgb = record.rgb();
                (rgb.r, rgb.g, rgb.b, record.count())
            })
            .collect::<Vec<_>>();
        values.sort_unstable();
        values
    }

    /// 生成每列颜色不同的横向渐变图。
    fn gradient_image(width: u32, height: u32) -> DynamicImage {
        let mut image = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(
                    x,
                    y,
                    Rgb([
                        x.wrapping_mul(17) as u8,
                        y.wrapping_mul(29) as u8,
                        x.wrapping_mul(7).wrapping_add(y.wrapping_mul(11)) as u8,
                    ]),
                );
            }
        }
        DynamicImage::ImageRgb8(image)
    }

    #[test]
    fn sampled_rejects_zero_and_respects_sample_limit() {
        let image = gradient_image(100, 1);
        let options = unmerged_options();

        assert!(load_by_image_sampled_with_options(&image, &options, 0).is_err());

        let (records, width, height) =
            load_by_image_sampled_with_options(&image, &options, 7).unwrap();
        assert_eq!(
            records.len(),
            7,
            "不同颜色的 7 个采样点应产生 7 条记录，不能超过采样上限"
        );
        assert_eq!((width, height), (100, 1), "必须返回原始图片尺寸");
        assert_eq!(
            records.iter().map(Record::count).sum::<u32>(),
            100,
            "加权记录必须覆盖全部原始像素"
        );
    }

    #[test]
    fn sampled_preserves_two_color_area_ratio() {
        let mut image = RgbImage::new(10, 1);
        for x in 0..10 {
            image.put_pixel(
                x,
                0,
                if x < 6 {
                    Rgb([255, 0, 0])
                } else {
                    Rgb([0, 0, 255])
                },
            );
        }

        let (records, width, height) = load_by_image_sampled_with_options(
            &DynamicImage::ImageRgb8(image),
            &unmerged_options(),
            3,
        )
        .unwrap();
        assert_eq!((width, height), (10, 1));
        assert_eq!(records.len(), 2);

        let red = records.iter().find(|record| record.rgb().r == 255).unwrap();
        let blue = records.iter().find(|record| record.rgb().b == 255).unwrap();
        assert_eq!(red.count(), 6);
        assert_eq!(blue.count(), 4);
    }

    #[test]
    fn small_image_is_equivalent_to_full_pixel_algorithm() {
        let image = gradient_image(7, 5);
        let options = unmerged_options().with_extract_max(16).with_output_max(16);

        let (full, full_width, full_height) = load_by_image_with_options(&image, &options).unwrap();
        let (sampled, sampled_width, sampled_height) =
            load_by_image_sampled_with_options(&image, &options, 35).unwrap();

        assert_eq!((sampled_width, sampled_height), (full_width, full_height));
        assert_eq!(normalized(&sampled), normalized(&full));
    }

    #[test]
    fn sampled_output_is_deterministic() {
        let image = gradient_image(37, 23);
        let options = unmerged_options().with_extract_max(32).with_output_max(32);

        let first = load_by_image_sampled_with_options(&image, &options, 31)
            .unwrap()
            .0;
        for _ in 0..5 {
            let next = load_by_image_sampled_with_options(&image, &options, 31)
                .unwrap()
                .0;
            assert_eq!(normalized(&next), normalized(&first));
            assert_eq!(
                next.iter()
                    .map(|record| (
                        record.rgb().r,
                        record.rgb().g,
                        record.rgb().b,
                        record.count()
                    ))
                    .collect::<Vec<_>>(),
                first
                    .iter()
                    .map(|record| (
                        record.rgb().r,
                        record.rgb().g,
                        record.rgb().b,
                        record.count()
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn sampled_handles_alpha_without_blackening_transparent_rgb() {
        let mut image = RgbaImage::new(4, 1);
        image.put_pixel(0, 0, Rgba([255, 0, 0, 0]));
        image.put_pixel(1, 0, Rgba([255, 0, 0, 0]));
        image.put_pixel(2, 0, Rgba([0, 0, 255, 128]));
        image.put_pixel(3, 0, Rgba([0, 0, 255, 128]));

        let records = load_by_image_sampled_with_options(
            &DynamicImage::ImageRgba8(image),
            &unmerged_options(),
            2,
        )
        .unwrap()
        .0;
        assert_eq!(normalized(&records), vec![(0, 0, 255, 2)]);
        assert!(
            !records.iter().any(|r| r.rgb().to_hex() == "#000000"),
            "全透明像素不应被当成黑色主色"
        );
    }

    #[test]
    fn full_path_skips_transparent_background() {
        let mut image = RgbaImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        for y in 4..6 {
            for x in 4..6 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let (records, width, height) = load_by_image_with_options(
            &DynamicImage::ImageRgba8(image),
            &unmerged_options(),
        )
        .unwrap();
        assert_eq!((width, height), (10, 10));
        assert_eq!(normalized(&records), vec![(255, 0, 0, 4)]);
        assert!(
            !records.iter().any(|r| r.rgb().to_hex() == "#000000"),
            "透明背景不应产生黑色主色"
        );
    }

    #[test]
    fn fully_transparent_image_returns_empty_palette() {
        let image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let (records, width, height) = load_by_image_with_options(
            &DynamicImage::ImageRgba8(image),
            &unmerged_options(),
        )
        .unwrap();
        assert_eq!((width, height), (8, 8));
        assert!(records.is_empty());
    }

    #[test]
    fn opaque_rgb_image_covers_all_pixels() {
        let mut image = RgbImage::new(10, 1);
        for x in 0..10 {
            image.put_pixel(
                x,
                0,
                if x < 6 {
                    Rgb([255, 0, 0])
                } else {
                    Rgb([0, 0, 255])
                },
            );
        }

        let (records, _, _) = load_by_image_with_options(
            &DynamicImage::ImageRgb8(image),
            &unmerged_options(),
        )
        .unwrap();
        assert_eq!(
            records.iter().map(Record::count).sum::<u32>(),
            10,
            "不透明 RGB 图 count 之和应等于全部像素"
        );
    }

    #[test]
    fn sampled_handles_sixteen_bit_images() {
        let mut image: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::new(4, 1);
        image.put_pixel(0, 0, Rgb([u16::MAX, 0, 0]));
        image.put_pixel(1, 0, Rgb([u16::MAX, 0, 0]));
        image.put_pixel(2, 0, Rgb([0, u16::MAX, 0]));
        image.put_pixel(3, 0, Rgb([0, u16::MAX, 0]));

        let records = load_by_image_sampled_with_options(
            &DynamicImage::ImageRgb16(image),
            &unmerged_options(),
            2,
        )
        .unwrap()
        .0;
        assert_eq!(normalized(&records), vec![(0, 255, 0, 2), (255, 0, 0, 2)]);
    }
}
