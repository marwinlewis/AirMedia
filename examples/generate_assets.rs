use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to encode PNG");
    buf.into_inner()
}

fn create_circle_icon(img: &RgbaImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut round = img.clone();
    let radius = (width.min(height) as f32) / 2.0;
    let cx = (width as f32) / 2.0;
    let cy = (height as f32) / 2.0;

    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 + 0.5) - cx;
            let dy = (y as f32 + 0.5) - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > radius {
                round.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if dist > radius - 1.0 {
                // Anti-aliased edge
                let alpha_scale = radius - dist;
                let mut p = *img.get_pixel(x, y);
                p.0[3] = ((p.0[3] as f32) * alpha_scale) as u8;
                round.put_pixel(x, y, p);
            }
        }
    }
    round
}

fn create_adaptive_foreground(src: &RgbaImage, target_size: u32) -> RgbaImage {
    let mut canvas = RgbaImage::new(target_size, target_size);
    // Safe zone is inner 66% (e.g. 72dp out of 108dp)
    let icon_size = ((target_size as f32) * 0.72).round() as u32;
    let scaled = image::imageops::resize(src, icon_size, icon_size, FilterType::Lanczos3);
    let offset = (target_size - icon_size) / 2;
    image::imageops::overlay(&mut canvas, &scaled, offset as i64, offset as i64);
    canvas
}

fn generate_ico(png_frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut ico = Vec::new();
    let count = png_frames.len() as u16;

    // 6-byte ICO Header
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Type 1 = Icon
    ico.extend_from_slice(&count.to_le_bytes()); // Image count

    let header_size = 6 + (16 * png_frames.len());
    let mut current_offset = header_size;

    // Directory entries (16 bytes each)
    for (size, data) in png_frames {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        ico.push(dim); // Width
        ico.push(dim); // Height
        ico.push(0); // Color count (0 = >=8bpp)
        ico.push(0); // Reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // Color planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes()); // Data size
        ico.extend_from_slice(&(current_offset as u32).to_le_bytes()); // Data offset
        current_offset += data.len();
    }

    // Image data payloads
    for (_, data) in png_frames {
        ico.extend_from_slice(data);
    }

    ico
}

fn generate_icns(png_frames: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut total_len = 8usize; // 4 bytes magic + 4 bytes length
    for (_, data) in png_frames {
        total_len += 8 + data.len();
    }

    let mut icns = Vec::with_capacity(total_len);
    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&(total_len as u32).to_be_bytes());

    for (ostype, data) in png_frames {
        icns.extend_from_slice(*ostype);
        let chunk_len = (8 + data.len()) as u32;
        icns.extend_from_slice(&chunk_len.to_be_bytes());
        icns.extend_from_slice(data);
    }

    icns
}

fn main() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        project_root.join("dist").join("airmedia.png"),
        project_root.join("assets").join("icon.png"),
        project_root.join("assets").join("linux").join("airmedia.png"),
    ];

    let source_icon_path = candidates
        .into_iter()
        .find(|p| p.exists())
        .expect("No source icon found in assets/icon.png or dist/airmedia.png");

    println!("Loading source icon from: {}", source_icon_path.display());
    let source_img = image::open(&source_icon_path)
        .expect("Failed to open source icon")
        .to_rgba8();

    let (src_w, src_h) = source_img.dimensions();
    println!("Source icon dimensions: {}x{}", src_w, src_h);

    // 1. Primary App Icons
    let assets_dir = project_root.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();

    let icon_512 = image::imageops::resize(&source_img, 512, 512, FilterType::Lanczos3);
    let icon_512_png = encode_png(&icon_512);
    let app_icon_path = assets_dir.join("icon.png");
    fs::write(&app_icon_path, &icon_512_png).unwrap();
    println!("Generated: {}", app_icon_path.display());

    // 2. Windows Assets
    let win_dir = assets_dir.join("windows");
    fs::create_dir_all(&win_dir).unwrap();

    let ico_sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let mut ico_frames = Vec::new();
    for size in ico_sizes {
        let resized = image::imageops::resize(&source_img, size, size, FilterType::Lanczos3);
        ico_frames.push((size, encode_png(&resized)));
    }
    let ico_bytes = generate_ico(&ico_frames);
    let ico_path = win_dir.join("icon.ico");
    fs::write(&ico_path, &ico_bytes).unwrap();
    println!("Generated: {} ({} frames)", ico_path.display(), ico_sizes.len());

    // 3. macOS Assets (AppIcon.icns)
    let macos_dir = assets_dir.join("macos");
    fs::create_dir_all(&macos_dir).unwrap();

    let icns_specs: &[(&[u8; 4], u32)] = &[
        (b"icp4", 16),
        (b"icp5", 32),
        (b"icp6", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic09", 512),
        (b"ic10", 1024),
    ];
    let mut icns_frames = Vec::new();
    for (ostype, size) in icns_specs {
        let resized = if *size == src_w && *size == src_h {
            source_img.clone()
        } else {
            image::imageops::resize(&source_img, *size, *size, FilterType::Lanczos3)
        };
        icns_frames.push((*ostype, encode_png(&resized)));
    }
    let icns_bytes = generate_icns(&icns_frames);
    let icns_path = macos_dir.join("AppIcon.icns");
    fs::write(&icns_path, &icns_bytes).unwrap();
    println!("Generated: {} ({} frames)", icns_path.display(), icns_specs.len());

    // 4. Linux Assets
    let linux_dir = assets_dir.join("linux");
    fs::create_dir_all(&linux_dir).unwrap();

    let linux_png = image::imageops::resize(&source_img, 256, 256, FilterType::Lanczos3);
    let linux_png_path = linux_dir.join("airmedia.png");
    fs::write(&linux_png_path, encode_png(&linux_png)).unwrap();
    println!("Generated: {}", linux_png_path.display());

    let hicolor_sizes = [16u32, 24, 32, 48, 64, 128, 256, 512];
    for size in hicolor_sizes {
        let size_dir = linux_dir
            .join("icons")
            .join("hicolor")
            .join(format!("{}x{}", size, size))
            .join("apps");
        fs::create_dir_all(&size_dir).unwrap();
        let resized = image::imageops::resize(&source_img, size, size, FilterType::Lanczos3);
        let out_file = size_dir.join("airmedia.png");
        fs::write(&out_file, encode_png(&resized)).unwrap();
    }
    println!("Generated Linux hicolor icon hierarchy (16 to 512)");

    // 5. Android Assets
    let android_assets_dir = assets_dir.join("android");
    fs::create_dir_all(&android_assets_dir).unwrap();

    // Standard Android mipmap specifications: (folder_name, launcher_size, foreground_size)
    let mipmap_specs = [
        ("mipmap-mdpi", 48u32, 108u32),
        ("mipmap-hdpi", 72, 162),
        ("mipmap-xhdpi", 96, 216),
        ("mipmap-xxhdpi", 144, 324),
        ("mipmap-xxxhdpi", 192, 432),
    ];

    for (folder, launcher_size, fg_size) in mipmap_specs {
        let target_dir = android_assets_dir.join(folder);
        fs::create_dir_all(&target_dir).unwrap();

        // 1. Standard square/adaptive icon
        let launcher_img = image::imageops::resize(&source_img, launcher_size, launcher_size, FilterType::Lanczos3);
        fs::write(target_dir.join("ic_launcher.png"), encode_png(&launcher_img)).unwrap();

        // 2. Round icon
        let round_img = create_circle_icon(&launcher_img);
        fs::write(target_dir.join("ic_launcher_round.png"), encode_png(&round_img)).unwrap();

        // 3. Adaptive foreground
        let fg_img = create_adaptive_foreground(&source_img, fg_size);
        fs::write(target_dir.join("ic_launcher_foreground.png"), encode_png(&fg_img)).unwrap();
    }

    // Google Play Store icon (512x512)
    let play_store_icon = image::imageops::resize(&source_img, 512, 512, FilterType::Lanczos3);
    let play_store_path = android_assets_dir.join("playstore-icon.png");
    fs::write(&play_store_path, encode_png(&play_store_icon)).unwrap();
    println!("Generated Android assets: mipmaps (mdpi to xxxhdpi) + playstore-icon.png");

    // Also mirror to android/ project if it exists
    let android_res_dir = project_root.join("android").join("app").join("src").join("main").join("res");
    if android_res_dir.exists() {
        for (folder, _, _) in mipmap_specs {
            let src_folder = android_assets_dir.join(folder);
            let dst_folder = android_res_dir.join(folder);
            fs::create_dir_all(&dst_folder).unwrap();
            for file in ["ic_launcher.png", "ic_launcher_round.png", "ic_launcher_foreground.png"] {
                let s = src_folder.join(file);
                let d = dst_folder.join(file);
                if s.exists() {
                    let _ = fs::copy(&s, &d);
                }
            }
        }
        let _ = fs::copy(&play_store_path, android_res_dir.join("ic_launcher-web.png"));
        println!("Mirrored Android icons to {}", android_res_dir.display());
    }

    let win_dist_dir = project_root.join("dist").join("airmedia-windows-x64");
    if win_dist_dir.exists() {
        let _ = fs::copy(&app_icon_path, win_dist_dir.join("icon.png"));
        println!("Mirrored icon to dist/airmedia-windows-x64/icon.png");
    }

    println!("\nAll assets successfully generated from dist/airmedia.png!");
}
