//! Renders `assets/logo.svg` into `assets/logo.png` (512 px) and a
//! multi-size `assets/kstar.ico`, the icon Windows shows for `kstar.exe` and
//! for `.kstar` files. Run it from the repository root after changing the SVG:
//!
//! ```text
//! cargo run --manifest-path tools/logo/Cargo.toml
//! ```

use std::fs::File;
use resvg::{tiny_skia, usvg};

/// The sizes Windows picks from, for lists, the taskbar and Explorer tiles.
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    let svg = std::fs::read("assets/logo.svg").expect("run this from the repository root");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("assets/logo.svg is not valid SVG");

    render(&tree, 512).save_png("assets/logo.png").expect("cannot write assets/logo.png");

    let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let pixmap = render(&tree, size);
        // tiny-skia stores premultiplied alpha; an icon wants it straight.
        let rgba: Vec<u8> = pixmap.pixels().iter().flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        }).collect();
        let image = ico::IconImage::from_rgba_data(size, size, rgba);
        icon.add_entry(ico::IconDirEntry::encode(&image).expect("cannot encode an icon image"));
    }
    icon.write(File::create("assets/kstar.ico").expect("cannot write assets/kstar.ico")).expect("cannot write assets/kstar.ico");
    println!("wrote assets/logo.png and assets/kstar.ico ({} sizes)", ICO_SIZES.len());
}

/// Draws the logo into a square of `size` pixels.
fn render(tree: &usvg::Tree, size: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}
