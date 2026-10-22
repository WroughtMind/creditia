//! 漫游缩略图在原生后台线程绘制。SVG 已由排版线程把文字转为路径，
//! WebView 只解码 PNG，不在移动时解析字形或计算纸纹滤镜。

fn png(svg: &str, width: u32) -> Result<Vec<u8>, String> {
    let mut options = resvg::usvg::Options::default();
    // 预览是自包含 SVG，只允许嵌入的图像，不解析本机文件引用。
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size().scale_to_width(width)
        .ok_or("Invalid preview dimensions")?;
    let mut pixels = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or("Cannot allocate preview image")?;
    let scale = width as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixels.as_mut());
    pixels.encode_png().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn render_preview(svg: String, width: u32) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || png(&svg, width))
        .await.map_err(|e| e.to_string())?.map(tauri::ipc::Response::new)
}

#[cfg(test)]
mod tests {
    use super::png;

    #[test]
    fn preview_preserves_transparency_and_scales_the_artwork() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect x="20" y="10" width="40" height="20" fill="#ff0000"/></svg>"##;
        let bytes = png(svg, 160).unwrap();
        let image = resvg::tiny_skia::Pixmap::decode_png(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), (160, 80));
        assert_eq!(image.pixel(0, 0).unwrap().alpha(), 0);
        let center = image.pixel(80, 40).unwrap();
        assert_eq!((center.red(), center.green(), center.blue(), center.alpha()), (255, 0, 0, 255));
    }

    #[test]
    fn preview_rejects_invalid_svg_and_empty_dimensions() {
        assert!(png("invalid", 160).is_err());
        assert!(png(r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"/>"#, 0).is_err());
    }

    #[test]
    fn preview_renders_embedded_images_but_never_reads_image_files() {
        use base64::Engine;
        let source = png(r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>"##, 10).unwrap();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("creditia-preview-{}-{stamp}.png",std::process::id()));
        std::fs::write(&path, &source).unwrap();
        let svg = |href: &str| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><image width="10" height="10" xlink:href="{href}"/></svg>"#);
        let external = png(&svg(path.to_str().unwrap()),10).unwrap();
        std::fs::remove_file(path).unwrap();
        let data = format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(source));
        let embedded = png(&svg(&data),10).unwrap();
        assert_eq!(resvg::tiny_skia::Pixmap::decode_png(&external).unwrap().pixel(5,5).unwrap().alpha(),0);
        assert_eq!(resvg::tiny_skia::Pixmap::decode_png(&embedded).unwrap().pixel(5,5).unwrap().alpha(),255);
    }
}
