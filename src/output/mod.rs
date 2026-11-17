//! 输出服务模块
//! 处理截图保存、剪贴板复制等操作

use std::path::{Path, PathBuf};

/// 文件名模板可用的时间片段
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeParts {
    pub date: String,
    pub time: String,
    pub datetime: String,
    pub unix: u64,
}

/// 读取本地时间
pub fn local_now() -> DateTimeParts {
    let now = chrono::Local::now();
    let date = now.format("%Y%m%d").to_string();
    let time = now.format("%H%M%S").to_string();
    let unix = now.timestamp().max(0) as u64;
    DateTimeParts { datetime: format!("{date}_{time}"), date, time, unix }
}

/// 渲染文件名模板
///
/// 支持 `{date}` `{time}` `{datetime}` `{unix}` `{index}`；
/// 空模板使用 `Screenshot_{datetime}`，非法字符替换为下划线。
pub fn render_template(template: &str, parts: &DateTimeParts, index: u32) -> String {
    let mut name = template.trim().to_string();
    if name.is_empty() {
        name = "Screenshot_{datetime}".to_string();
    }
    let name = name
        .replace("{date}", &parts.date)
        .replace("{time}", &parts.time)
        .replace("{datetime}", &parts.datetime)
        .replace("{unix}", &parts.unix.to_string())
        .replace("{index}", &format!("{index:03}"));
    sanitize_filename(&name)
}

fn sanitize_filename(name: &str) -> String {
    const INVALID: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let cleaned: String = name.chars().map(|c| if INVALID.contains(&c) { '_' } else { c }).collect();
    let trimmed = cleaned.trim_matches(|c: char| c == ' ' || c == '.').to_string();
    if trimmed.is_empty() {
        "Screenshot".to_string()
    } else {
        trimmed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
}

impl ImageFormat {
    pub fn from_ext(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "bmp" => Some(Self::Bmp),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Bmp => "bmp",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SaveOptions {
    pub path: PathBuf,
    pub format: ImageFormat,
    /// JPEG 质量 1-100
    pub quality: u8,
}

/// 输出服务
pub struct OutputService;

impl OutputService {
    /// 保存图像（输入为 BGRA 像素缓冲区）
    pub fn save_image(pixels: &[u8], width: u32, height: u32, options: &SaveOptions) -> anyhow::Result<PathBuf> {
        let rgba = crate::utils::bgra_to_rgba(pixels);
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| anyhow::anyhow!("像素缓冲区与图像尺寸不匹配"))?;

        if let Some(parent) = options.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        match options.format {
            ImageFormat::Png => image.save(&options.path)?,
            ImageFormat::Bmp => image.save(&options.path)?,
            ImageFormat::Jpeg => {
                let rgb = image::DynamicImage::ImageRgba8(image).to_rgb8();
                let file = std::fs::File::create(&options.path)?;
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    std::io::BufWriter::new(file),
                    options.quality.clamp(1, 100),
                );
                encoder.encode_image(&rgb)?;
            }
        }

        Ok(options.path.clone())
    }

    /// 复制图像到系统剪贴板（输入为 BGRA 像素缓冲区）
    pub fn copy_to_clipboard(pixels: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
        let rgba = crate::utils::bgra_to_rgba(pixels);
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into(),
        })?;
        Ok(())
    }

    /// 生成文件名（基于本地时间戳）
    pub fn generate_filename(format: &ImageFormat) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("screenshot_{ts}.{}", format.extension())
    }

    /// 保存到系统临时目录，返回完整路径
    pub fn save_to_temp(pixels: &[u8], width: u32, height: u32, format: ImageFormat) -> anyhow::Result<PathBuf> {
        let path = std::env::temp_dir().join(Self::generate_filename(&format));
        Self::save_image(pixels, width, height, &SaveOptions { path, format, quality: 92 })
    }

    /// 按保存目录 + 文件名模板保存，自动避免重名覆盖
    pub fn save_with_template(
        pixels: &[u8],
        width: u32,
        height: u32,
        dir: &Path,
        template: &str,
        format: ImageFormat,
        quality: u8,
    ) -> anyhow::Result<PathBuf> {
        let parts = local_now();
        let stem = render_template(template, &parts, 0);
        let path = unique_path(dir, &stem, format.extension());
        Self::save_image(pixels, width, height, &SaveOptions { path, format, quality })
    }

    /// 确保目录存在（供自定义保存路径使用）
    pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

/// 生成不重名的保存路径（重名时追加 _2、_3…）
fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let has_extension = Path::new(stem).extension().is_some();
    let base = if has_extension { dir.join(stem) } else { dir.join(format!("{stem}.{ext}")) };
    if !base.exists() {
        return base;
    }

    let file_stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or(stem).to_string();
    let actual_ext = base.extension().and_then(|s| s.to_str()).unwrap_or(ext).to_string();

    let mut index = 2;
    loop {
        let candidate = dir.join(format!("{file_stem}_{index}.{actual_ext}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_parts() -> DateTimeParts {
        DateTimeParts {
            date: "20261117".to_string(),
            time: "153045".to_string(),
            datetime: "20261117_153045".to_string(),
            unix: 1_794_850_000,
        }
    }

    #[test]
    fn template_renders_date_time_and_index() {
        let parts = sample_parts();
        assert_eq!(render_template("Shot_{date}_{time}", &parts, 0), "Shot_20261117_153045");
        assert_eq!(render_template("Screenshot_{datetime}_{index}", &parts, 3), "Screenshot_20261117_153045_003");
        assert_eq!(render_template("cap_{unix}", &parts, 0), "cap_1794850000");
        assert_eq!(render_template("", &parts, 0), "Screenshot_20261117_153045");
    }

    #[test]
    fn template_sanitizes_invalid_filename_chars() {
        let parts = sample_parts();
        let name = render_template("a/b:c*?\"<>|d", &parts, 0);
        assert!(!name.contains('/'));
        assert!(!name.contains(':'));
        assert!(!name.contains('*'));
        assert!(!name.contains('?'));
        assert!(name.starts_with("a_b_c"));
    }

    #[test]
    fn unique_path_avoids_overwrite() {
        let dir = std::env::temp_dir().join("screencapture_unique_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建目录应成功");

        let first = unique_path(&dir, "shot", "png");
        std::fs::write(&first, b"x").expect("写入应成功");
        let second = unique_path(&dir, "shot", "png");

        assert_ne!(first, second);
        assert!(second.to_string_lossy().contains("shot_2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_keeps_unknown_tokens_and_unicode() {
        let parts = sample_parts();
        assert_eq!(render_template("截图_{date}_{unknown}", &parts, 0), "截图_20261117_{unknown}");
    }

    #[test]
    fn unique_path_respects_extension_in_template() {
        let dir = std::env::temp_dir().join("screencapture_ext_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建目录应成功");

        let first = unique_path(&dir, "shot.png", "png");
        assert_eq!(first.file_name().and_then(|n| n.to_str()), Some("shot.png"));

        std::fs::write(&first, b"x").expect("写入应成功");
        let second = unique_path(&dir, "shot.png", "png");
        assert_eq!(second.file_name().and_then(|n| n.to_str()), Some("shot_2.png"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_png_roundtrips_pixels() {
        let dir = std::env::temp_dir().join("screencapture_png_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("pixel.png");

        // BGRA：纯红 + 纯白
        let pixels = vec![0u8, 0, 255, 255, 255, 255, 255, 255];
        OutputService::save_image(
            &pixels,
            2,
            1,
            &SaveOptions { path: path.clone(), format: ImageFormat::Png, quality: 92 },
        )
        .expect("保存 PNG 应成功");

        let decoded = image::open(&path).expect("应能解码 PNG").to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [255, 255, 255, 255]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn filename_has_extension() {
        let name = OutputService::generate_filename(&ImageFormat::Png);
        assert!(name.ends_with(".png"));
        assert!(name.starts_with("screenshot_"));
    }

    #[test]
    fn save_png_roundtrip() {
        let width = 4u32;
        let height = 3u32;
        let pixels = vec![0u8; (width * height * 4) as usize];
        let path = std::env::temp_dir().join("screencapture_test_output.png");
        let saved = OutputService::save_image(
            &pixels,
            width,
            height,
            &SaveOptions { path: path.clone(), format: ImageFormat::Png, quality: 90 },
        )
        .expect("保存 PNG 应成功");
        assert!(saved.exists());
        let _ = std::fs::remove_file(saved);
    }

    #[test]
    fn format_parsing() {
        assert_eq!(ImageFormat::from_ext("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_ext("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_ext("gif"), None);
    }
}
