use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc, str::FromStr};

use error::ImageError;
use image::{
    DynamicImage,
    ImageError::{IoError, Unsupported},
    RgbImage, RgbaImage,
};

mod error;

/// Open the image located at the path specified, return 16 dominant colors.
///
/// # Examples
/// ```no_run
/// let (colors, width, height) = image_palette::load("test.jpg").unwrap();
/// println!("total: {}", width * height);
/// for color in colors {
///   println!("{}: {}", color.rgb().to_hex(), color.count());
/// }
/// ```
pub fn load<P>(path: P) -> Result<(Vec<Record>, u32, u32), ImageError>
where
    P: AsRef<Path>,
{
    OcTree::load_with_maxcolor(path, 16)
}

/// Open the image located at the path specified, return {max_color} dominant colors.
///
/// # Examples
/// ```no_run
/// let (colors, width, height) = image_palette::load_with_maxcolor("test.jpg", 8).unwrap();
/// println!("total: {}", width * height);
/// for color in colors {
///   println!("{}: {}", color.rgb().to_hex(), color.count());
/// }
/// ```
pub fn load_with_maxcolor<P>(path: P, max_color: u8) -> Result<(Vec<Record>, u32, u32), ImageError>
where
    P: AsRef<Path>,
{
    OcTree::load_with_maxcolor(path, max_color)
}

/// Open the image with image crate, return {max_color} dominant colors.
///
/// # Examples
/// ```no_run
/// let image = image::open("test.jpg").unwrap();
/// let (colors, width, height) = image_palette::load_by_image_with_maxcolor(&image, 8).unwrap();
/// println!("total: {}", width * height);
/// for color in colors {
///   println!("{}: {}", color.rgb().to_hex(), color.count());
/// }
/// ```
pub fn load_by_image_with_maxcolor(
    image: &DynamicImage,
    max_color: u8,
) -> Result<(Vec<Record>, u32, u32), ImageError> {
    // 等价于旧行为：不合并、不按占比过滤、不截断输出数量
    let options = PaletteOptions {
        extract_max: max_color,
        output_max: max_color,
        merge_delta_e: 0.0,
        min_ratio: 0.0,
    };
    load_by_image_with_options(image, &options)
}

/// 主色提取配置。
///
/// `extract_max` 控制 OctTree 内部 bin 数（越大越能区分相近色）；
/// `output_max` 控制最终返回的主色数量上限；
/// `merge_delta_e` 为 CIELAB ΔE 合并阈值，`0.0` 表示不合并（需启用 `lab` feature 才生效）；
/// `min_ratio` 为最小像素占比（百分比），低于此值的颜色会被丢弃。
#[derive(Debug, Clone, Copy)]
pub struct PaletteOptions {
    pub extract_max: u8,
    pub output_max: u8,
    pub merge_delta_e: f32,
    pub min_ratio: f32,
}

impl Default for PaletteOptions {
    fn default() -> Self {
        Self {
            extract_max: 16,
            output_max: 8,
            merge_delta_e: 10.0,
            min_ratio: 0.01,
        }
    }
}

impl PaletteOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_extract_max(mut self, v: u8) -> Self {
        self.extract_max = v;
        self
    }
    pub fn with_output_max(mut self, v: u8) -> Self {
        self.output_max = v;
        self
    }
    pub fn with_merge_delta_e(mut self, v: f32) -> Self {
        self.merge_delta_e = v;
        self
    }
    pub fn with_min_ratio(mut self, v: f32) -> Self {
        self.min_ratio = v;
        self
    }
}

/// 按配置提取主色：OctTree 提取 → 相近色合并（可选）→ 占比过滤 → 数量截断。
///
/// # Examples
/// ```no_run
/// let image = image::open("test.jpg").unwrap();
/// let opts = image_palette::PaletteOptions::default();
/// let (colors, width, height) = image_palette::load_by_image_with_options(&image, &opts).unwrap();
/// for color in &colors {
///   println!("{}: {}", color.rgb().to_hex(), color.count());
/// }
/// ```
pub fn load_by_image_with_options(
    image: &DynamicImage,
    options: &PaletteOptions,
) -> Result<(Vec<Record>, u32, u32), ImageError> {
    let (mut list, width, height) = OcTree::load_by_image(image, options.extract_max as u32);

    #[cfg(feature = "lab")]
    {
        if options.merge_delta_e > 0.0 {
            list = merge_similar(list, options.merge_delta_e);
        }
    }
    #[cfg(not(feature = "lab"))]
    {
        if options.merge_delta_e > 0.0 {
            return Err(ImageError::InvalidParameter);
        }
    }

    let total = (width * height) as f32;
    if options.min_ratio > 0.0 {
        list.retain(|r| r.count as f32 / total * 100.0 >= options.min_ratio);
    }
    list.sort_by(|a, b| b.count.cmp(&a.count));
    if list.len() > options.output_max as usize {
        list.truncate(options.output_max as usize);
    }
    Ok((list, width, height))
}

/// 基于 CIELAB ΔE 的相近色层次聚类合并（质心链接）。
///
/// 每个 OctTree 色先各成一簇，反复合并当前距离最小的两簇，
/// 直到最近距离超过 `delta_e`。距离用折算明度的 CIE76：
///
/// ```text
/// ΔE' = sqrt((ΔL / 2)^2 + Δa^2 + Δb^2)
/// ```
///
/// 明度差权重减半，让同色相的明度渐变（如纸纹的浅棕→深棕）
/// 收成少数主色，而不是凑满 8 个；红/蓝等不同色相主要在 a/b 轴，不受影响。
/// 合并时 RGB 与 Lab 均按像素 count 加权平均。
#[cfg(feature = "lab")]
fn merge_similar(records: Vec<Record>, delta_e: f32) -> Vec<Record> {
    if delta_e <= 0.0 || records.is_empty() {
        return records;
    }

    // 簇：(sum_r*count, sum_g*count, sum_b*count, count, sum_l*count, sum_a*count, sum_b_lab*count)
    // RGB 必须按像素 count 加权累加，否则回除 count 后会得到接近 #000000 的错误均值
    let mut clusters: Vec<(u64, u64, u64, u32, f64, f64, f64)> = records
        .into_iter()
        .map(|rec| {
            let lab = rec.rgb.to_lab();
            let count = rec.count as u64;
            (
                rec.rgb.r as u64 * count,
                rec.rgb.g as u64 * count,
                rec.rgb.b as u64 * count,
                rec.count,
                lab.l as f64 * rec.count as f64,
                lab.a as f64 * rec.count as f64,
                lab.b as f64 * rec.count as f64,
            )
        })
        .collect();

    // 反复合并当前 ΔE' 最小的两簇，直到最小距离超过阈值
    loop {
        let mut best: Option<(usize, usize, f32)> = None;
        for i in 0..clusters.len() {
            let ci = &clusters[i];
            let ci_cnt = ci.3 as f64;
            let cil = ci.4 / ci_cnt;
            let cia = ci.5 / ci_cnt;
            let cib = ci.6 / ci_cnt;
            for j in (i + 1)..clusters.len() {
                let cj = &clusters[j];
                let cj_cnt = cj.3 as f64;
                let cjl = cj.4 / cj_cnt;
                let cja = cj.5 / cj_cnt;
                let cjb = cj.6 / cj_cnt;
                let dl = (cil - cjl) / 2.0;
                let da = cia - cja;
                let db = cib - cjb;
                let dist = ((dl * dl + da * da + db * db) as f32).sqrt();
                match best {
                    None => best = Some((i, j, dist)),
                    Some((_, _, d)) if dist < d => best = Some((i, j, dist)),
                    _ => {}
                }
            }
        }

        match best {
            Some((i, j, dist)) if dist <= delta_e => {
                // 把 j 并入 i（i < j），然后丢弃 j
                let cj = clusters[j].clone();
                let ci = &mut clusters[i];
                ci.0 += cj.0;
                ci.1 += cj.1;
                ci.2 += cj.2;
                ci.3 += cj.3;
                ci.4 += cj.4;
                ci.5 += cj.5;
                ci.6 += cj.6;
                clusters.remove(j);
            }
            _ => break,
        }
    }

    let mut result: Vec<Record> = clusters
        .into_iter()
        .map(|c| {
            let cnt = c.3 as u64;
            // 四舍五入，避免整数截断把浅色系统性压暗
            let r = ((c.0 + cnt / 2) / cnt) as u8;
            let g = ((c.1 + cnt / 2) / cnt) as u8;
            let b = ((c.2 + cnt / 2) / cnt) as u8;
            Record {
                rgb: RGB { r, g, b },
                count: c.3,
            }
        })
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result
}

#[derive(Debug)]
struct OcTree {
    leaf_num: u32,
    to_reduce: [Vec<Rc<RefCell<Node>>>; 8],
    max_color: u32,
}

impl OcTree {
    fn load_with_maxcolor<P>(path: P, max_color: u8) -> Result<(Vec<Record>, u32, u32), ImageError>
    where
        P: AsRef<Path>,
    {
        let image = image::open(path).map_err(|error| match error {
            Unsupported(error) => ImageError::UnsupportedFile(error),
            IoError(error) => ImageError::IoError(error),
            error => ImageError::Unknown(error),
        })?;

        Ok(Self::load_by_image(&image, max_color.into()))
    }

    fn load_by_image(image: &DynamicImage, max_color: u32) -> (Vec<Record>, u32, u32) {
        const ARRAY_REPEAT_VALUE: Vec<Rc<RefCell<Node>>> = Vec::new();
        let mut tree = OcTree {
            leaf_num: 0,
            to_reduce: [ARRAY_REPEAT_VALUE; 8],
            max_color,
        };

        let rgb = image.to_rgb8();
        let image_data = ImageData::from(&rgb);

        let root_share = tree.create_node(0);

        for color in image_data.data {
            tree.add_color(&root_share, color, 0);
            while tree.leaf_num > tree.max_color {
                tree.reduce_tree();
            }
        }

        let mut map: HashMap<RGB, u32> = HashMap::new();
        colors_stats(&root_share, &mut map);
        let mut list = Vec::new();
        for (rgb, count) in map {
            list.push(Record { rgb, count });
        }
        list.sort_by(|a, b| b.count.cmp(&a.count));
        (list, image_data.width, image_data.height)
    }

    fn create_node(&mut self, level: usize) -> Rc<RefCell<Node>> {
        let node = Node::new();
        let node_share: Rc<RefCell<Node>> = Rc::new(RefCell::new(node));

        if level == 7 {
            let mut node_mut: std::cell::RefMut<Node> = node_share.borrow_mut();
            node_mut.is_leaf = true;
            self.leaf_num += 1;
        } else {
            let a: Rc<RefCell<Node>> = Rc::clone(&node_share);
            self.to_reduce[level].push(a);
            self.to_reduce[level].sort_by_key(|k: &Rc<RefCell<Node>>| k.borrow().pixel_count);
        }

        node_share
    }

    fn add_color(&mut self, node_share: &Rc<RefCell<Node>>, rgb: RGB, level: usize) {
        let mut node: std::cell::RefMut<Node> = node_share.borrow_mut();
        if node.is_leaf {
            node.pixel_count += 1;
            node.r += rgb.r as u32;
            node.g += rgb.g as u32;
            node.b += rgb.b as u32;
        } else {
            let r = rgb.r >> (7 - level) & 1;
            let g = rgb.g >> (7 - level) & 1;
            let b = rgb.b >> (7 - level) & 1;

            let idx = ((r << 2) + (g << 1) + b) as usize;

            if node.children[idx].is_none() {
                let child_share: Rc<RefCell<Node>> = self.create_node(level + 1);
                node.children[idx] = Some(child_share);
            }

            self.add_color(node.children[idx].as_ref().unwrap(), rgb, level + 1);
        }
    }

    fn reduce_tree(&mut self) {
        // find the deepest level of node
        let mut lv: isize = 6;

        while lv >= 0 && self.to_reduce[lv as usize].len() == 0 {
            lv -= 1;
        }
        if lv < 0 {
            return;
        }

        let node_share = self.to_reduce[lv as usize].pop().unwrap();
        let mut node = node_share.borrow_mut();

        // merge children
        let mut r = 0;
        let mut g = 0;
        let mut b = 0;
        let mut pixel_count = 0;

        for i in 0..8 {
            if node.children[i].is_none() {
                continue;
            }
            let child_share = node.children[i].as_ref().unwrap();
            let child = child_share.borrow();

            r += child.r;
            g += child.g;
            b += child.b;
            pixel_count += child.pixel_count;
            self.leaf_num -= 1;
        }

        node.is_leaf = true;
        node.r = r;
        node.g = g;
        node.b = b;
        node.pixel_count = pixel_count;

        self.leaf_num += 1;
    }
}

fn colors_stats(node_share: &Rc<RefCell<Node>>, map: &mut HashMap<RGB, u32>) {
    let node = node_share.borrow_mut();
    if node.is_leaf {
        let r = (node.r / node.pixel_count) as u8;
        let g = (node.g / node.pixel_count) as u8;
        let b = (node.b / node.pixel_count) as u8;
        let rgb = RGB::from(&[r, g, b]);
        if let Some(x) = map.get_mut(&rgb) {
            *x = *x + node.pixel_count;
        } else {
            map.insert(rgb, node.pixel_count);
        }
    } else {
        for i in 0..8 {
            if node.children[i].is_some() {
                colors_stats(node.children[i].as_ref().unwrap(), map);
            }
        }
    }
}

impl From<&RgbImage> for ImageData {
    fn from(image: &RgbImage) -> Self {
        let (width, height) = image.dimensions();
        let size = (width * height) as usize;

        let data = image
            .pixels()
            .fold(Vec::with_capacity(size), |mut pixels, pixel| {
                pixels.push(RGB::from(&[pixel[0], pixel[1], pixel[2]]));
                pixels
            });

        Self {
            data,
            width,
            height,
        }
    }
}

impl From<&RgbaImage> for ImageData {
    fn from(image: &RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        let size = (width * height) as usize;

        let data = image.pixels().filter(|pixels| pixels[3] > 0).fold(
            Vec::with_capacity(size),
            |mut pixels, pixel| {
                pixels.push(RGB::from(&[pixel[0], pixel[1], pixel[2]]));
                pixels
            },
        );

        Self {
            data,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGB {
    pub fn from(rgb: &[u8; 3]) -> RGB {
        RGB {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
        }
    }

    pub fn to_hex(&self) -> String {
        let r = format!("{:0>2}", format!("{:X}", self.r));
        let g = format!("{:0>2}", format!("{:X}", self.g));
        let b = format!("{:0>2}", format!("{:X}", self.b));
        format!("#{}{}{}", r, g, b)
    }

    #[cfg(feature = "lab")]
    pub fn to_lab(&self) -> lab::Lab {
        lab::Lab::from_rgb(&[self.r, self.g, self.b])
    }
}

impl FromStr for RGB {
    type Err = std::num::ParseIntError;

    fn from_str(hex_code: &str) -> Result<Self, Self::Err> {
        let r: u8 = u8::from_str_radix(&hex_code[1..3], 16)?;
        let g: u8 = u8::from_str_radix(&hex_code[3..5], 16)?;
        let b: u8 = u8::from_str_radix(&hex_code[5..7], 16)?;

        Ok(RGB { r, g, b })
    }
}

struct ImageData {
    data: Vec<RGB>,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct Node {
    is_leaf: bool,
    r: u32,
    g: u32,
    b: u32,
    pixel_count: u32,
    children: [Option<Rc<RefCell<Node>>>; 8],
}

impl Node {
    fn new() -> Node {
        const ARRAY_REPEAT_VALUE: Option<Rc<RefCell<Node>>> = None;
        Node {
            is_leaf: false,
            r: 0,
            g: 0,
            b: 0,
            pixel_count: 0,
            children: [ARRAY_REPEAT_VALUE; 8],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Record {
    rgb: RGB,
    count: u32,
}

impl Record {
    pub fn new(rgb: RGB, count: u32) -> Self {
        Record { rgb, count }
    }
    pub fn rgb(&self) -> &RGB {
        &self.rgb
    }
    pub fn count(&self) -> u32 {
        self.count
    }
}
