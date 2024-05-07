use std::{cell::RefCell, collections::HashMap, rc::Rc};

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
/// let colors = image_palette::load("test.jpg").unwrap();
///
/// for item in colors {
///   println!("{}:{}", item.color(), item.count());
/// }
/// ```
pub fn load(path: &str) -> Result<Vec<Record>, ImageError> {
    OcTree::load_with_maxcolor(path, 16)
}

/// Open the image located at the path specified, return {max_color} dominant colors.
///
/// # Examples
/// ```
/// let colors = image_palette::load_with_maxcolor("test.jpg", 32).unwrap();
///
/// for item in colors {
///   println!("{}:{}", item.color(), item.count());
/// }
/// ```
pub fn load_with_maxcolor(path: &str, max_color: u32) -> Result<Vec<Record>, ImageError> {
    OcTree::load_with_maxcolor(path, max_color)
}

#[derive(Debug)]
struct OcTree {
    leaf_num: u32,
    to_reduce: [Vec<Rc<RefCell<Node>>>; 8],
    max_color: u32,
}

impl OcTree {
    fn load_with_maxcolor(path: &str, max_color: u32) -> Result<Vec<Record>, ImageError> {
        const ARRAY_REPEAT_VALUE: Vec<Rc<RefCell<Node>>> = Vec::new();
        let mut tree = OcTree {
            leaf_num: 0,
            to_reduce: [ARRAY_REPEAT_VALUE; 8],
            max_color,
        };

        let image = image::open(&path).map_err(|error| match error {
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

        let mut map: HashMap<String, u32> = HashMap::new();
        colors_stats(&root_share, &mut map);
        let mut list = Vec::new();
        for (color, count) in map {
            list.push(Record { color, count });
        }
        list.sort_by(|a, b| b.count.cmp(&a.count));
        Ok(list)
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

    fn add_color(&mut self, node_share: &Rc<RefCell<Node>>, color: Color, level: usize) {
        let mut node: std::cell::RefMut<Node> = node_share.borrow_mut();
        if node.is_leaf {
            node.pixel_count += 1;
            node.r += color.0 as u32;
            node.g += color.1 as u32;
            node.b += color.2 as u32;
        } else {
            let r = color.0 >> (7 - level) & 1;
            let g = color.1 >> (7 - level) & 1;
            let b = color.2 >> (7 - level) & 1;

            let idx = ((r << 2) + (g << 1) + b) as usize;

            if node.children[idx].is_none() {
                let child_share: Rc<RefCell<Node>> = self.create_node(level + 1);
                node.children[idx] = Some(child_share);
            }

            self.add_color(node.children[idx].as_ref().unwrap(), color, level + 1);
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

fn colors_stats(node_share: &Rc<RefCell<Node>>, map: &mut HashMap<String, u32>) {
    let node = node_share.borrow_mut();
    if node.is_leaf {
        let r = format!("{:0>2}", format!("{:X}", node.r / node.pixel_count));
        let g = format!("{:0>2}", format!("{:X}", node.g / node.pixel_count));
        let b = format!("{:0>2}", format!("{:X}", node.b / node.pixel_count));
        let color = format!("#{}{}{}", r, g, b);
        if let Some(x) = map.get_mut(&color) {
            *x = *x + node.pixel_count;
        } else {
            map.insert(color, node.pixel_count);
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
                pixels.push(Color(pixel[0], pixel[1], pixel[2]));
                pixels
            });

        Self { data }
    }
}

impl From<&RgbaImage> for ImageData {
    fn from(image: &RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        let size = (width * height) as usize;

        let data = image.pixels().filter(|pixels| pixels[3] > 0).fold(
            Vec::with_capacity(size),
            |mut pixels, pixel| {
                pixels.push(Color(pixel[0], pixel[1], pixel[2]));
                pixels
            },
        );

        Self { data }
    }
}

struct ImageData {
    data: Vec<Color>,
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
struct Color(u8, u8, u8);

#[derive(Debug)]
pub struct Record {
    color: String,
    count: u32,
}

impl Record {
    pub fn color(&self) -> &str {
        &self.color
    }
    pub fn count(&self) -> u32 {
        self.count
    }
}
