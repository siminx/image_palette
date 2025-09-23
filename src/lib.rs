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
/// ```
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
/// ```
/// let (colors, width, height) = image_palette::load_with_maxcolor("test.jpg", 8).unwrap();
/// println!("total: {}", width * height);
/// for color in colors {
///   println!("{}: {}", color.rgb().to_hex(), color.count());
/// }
/// ```
pub fn load_with_maxcolor<P>(path: P, max_color: u32) -> Result<(Vec<Record>, u32, u32), ImageError>
where
    P: AsRef<Path>,
{
    OcTree::load_with_maxcolor(path, max_color)
}

#[derive(Debug)]
struct OcTree {
    leaf_num: u32,
    to_reduce: [Vec<Rc<RefCell<Node>>>; 8],
    max_color: u32,
}

impl OcTree {
    fn load_with_maxcolor<P>(path: P, max_color: u32) -> Result<(Vec<Record>, u32, u32), ImageError>
    where
        P: AsRef<Path>,
    {
        const ARRAY_REPEAT_VALUE: Vec<Rc<RefCell<Node>>> = Vec::new();
        let mut tree = OcTree {
            leaf_num: 0,
            to_reduce: [ARRAY_REPEAT_VALUE; 8],
            max_color,
        };

        let image = image::open(path).map_err(|error| match error {
            Unsupported(error) => ImageError::UnsupportedFile(error),
            IoError(error) => ImageError::IoError(error),
            error => ImageError::Unknown(error),
        })?;

        let image_data = ImageData::try_from(&image)?;

        let root = Node::new();
        let root_share: Rc<RefCell<Node>> = Rc::new(RefCell::new(root));

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
        Ok((list, image_data.width, image_data.height))
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

impl TryFrom<&DynamicImage> for ImageData {
    type Error = ImageError;

    fn try_from(image: &DynamicImage) -> Result<Self, Self::Error> {
        match image {
            image::DynamicImage::ImageRgb8(image) => Ok(ImageData::from(image)),
            image::DynamicImage::ImageRgba8(image) => Ok(ImageData::from(image)),
            _ => Err(ImageError::UnsupportedType(image.color())),
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
    r: u8,
    g: u8,
    b: u8,
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

#[derive(Debug)]
pub struct Record {
    rgb: RGB,
    count: u32,
}

impl Record {
    pub fn rgb(&self) -> &RGB {
        &self.rgb
    }
    pub fn count(&self) -> u32 {
        self.count
    }
}
