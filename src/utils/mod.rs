//! 通用工具模块

/// 像素格式转换：BGRA -> RGBA
pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let (pixels, _rest) = bgra.as_chunks::<4>();
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in pixels {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    rgba
}

/// 像素格式转换：RGBA -> BGRA
pub fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let (pixels, _rest) = rgba.as_chunks::<4>();
    let mut bgra = Vec::with_capacity(rgba.len());
    for pixel in pixels {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    bgra
}

/// 限制值在范围内
pub fn clamp<T: Ord + Copy>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgra_to_rgba() {
        let bgra = vec![10, 20, 30, 255]; // B=10, G=20, R=30, A=255
        let rgba = bgra_to_rgba(&bgra);
        assert_eq!(rgba, vec![30, 20, 10, 255]);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-1, 0, 10), 0);
        assert_eq!(clamp(100, 0, 10), 10);
    }
}
