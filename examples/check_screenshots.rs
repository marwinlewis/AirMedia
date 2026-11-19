use std::path::Path;

fn main() {
    let path = Path::new("docs/screenshots/details.png");
    let img = image::open(&path).unwrap().to_rgba8();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            if p[0] >= 220 && p[1] >= 60 && p[1] <= 140 && p[2] >= 30 && p[2] <= 90 {
                // flame pixel
                println!("details.png flame at ({}, {}) color: {:?}", x, y, p);
                return;
            }
        }
    }
}
