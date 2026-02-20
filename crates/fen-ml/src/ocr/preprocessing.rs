use image::{DynamicImage, GrayImage, Luma};
use imageproc::contrast::{equalize_histogram, otsu_level, stretch_contrast};
use imageproc::filter::gaussian_blur_f32;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use serde::{Deserialize, Serialize};

/// Configuration for image preprocessing steps applied before OCR detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingConfig {
    /// Enable the preprocessing pipeline (default: false — opt-in)
    pub enabled: bool,

    /// Deskew: detect and correct document rotation
    pub deskew: bool,

    /// Maximum rotation angle to correct (degrees)
    pub deskew_max_angle: f32,

    /// Denoise: apply Gaussian blur to reduce noise
    pub denoise: bool,

    /// Gaussian blur sigma for denoising
    pub denoise_sigma: f32,

    /// Contrast enhancement before OCR
    pub contrast_enhancement: bool,

    /// Contrast enhancement strategy
    pub contrast_strategy: ContrastStrategy,

    /// Binarize: convert to black/white (aggressive — not always desired)
    pub binarize: bool,

    /// Binarization strategy
    pub binarization_strategy: BinarizationStrategy,

    /// Block size for adaptive thresholding (must be odd)
    pub adaptive_block_size: u32,
}

impl Default for PreprocessingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            deskew: true,
            deskew_max_angle: 15.0,
            denoise: true,
            denoise_sigma: 1.0,
            contrast_enhancement: true,
            contrast_strategy: ContrastStrategy::Stretch,
            binarize: false,
            binarization_strategy: BinarizationStrategy::Otsu,
            adaptive_block_size: 15,
        }
    }
}

/// Contrast enhancement strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContrastStrategy {
    /// Stretch pixel range to fill [0, 255]
    Stretch,
    /// Histogram equalization for uniform distribution
    HistogramEqualization,
}

/// Binarization strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinarizationStrategy {
    /// Otsu's method — global threshold
    Otsu,
    /// Adaptive thresholding — local threshold per block
    Adaptive,
}

/// Apply configured preprocessing steps to an image.
/// Returns an RGB image suitable for the OCR pipeline.
pub fn preprocess_image(image: &DynamicImage, config: &PreprocessingConfig) -> DynamicImage {
    if !config.enabled {
        return image.clone();
    }

    let mut working = image.clone();

    // Step 1: Deskew (detect angle on grayscale, apply to RGB)
    if config.deskew {
        working = deskew(&working, config.deskew_max_angle);
    }

    // Step 2: Denoise (Gaussian blur on grayscale, convert back to RGB)
    if config.denoise {
        working = denoise_image(&working, config.denoise_sigma);
    }

    // Step 3: Contrast enhancement
    if config.contrast_enhancement {
        working = enhance_contrast(&working, config.contrast_strategy);
    }

    // Step 4: Binarization (optional, aggressive)
    if config.binarize {
        working = binarize_image(&working, config.binarization_strategy, config.adaptive_block_size);
    }

    working
}

/// Detect skew angle using projection profile method and correct it.
///
/// Algorithm:
/// 1. Convert to grayscale, Otsu threshold to binary
/// 2. Downsample for performance
/// 3. Test candidate angles, maximize sum-of-squares of horizontal row projections
/// 4. Rotate original image by detected angle
fn deskew(image: &DynamicImage, max_angle: f32) -> DynamicImage {
    let gray = image.to_luma8();
    let (w, h) = (gray.width(), gray.height());

    // Downsample large images for angle detection performance
    let max_detection_height = 512u32;
    let (detect_gray, scale) = if h > max_detection_height {
        let s = max_detection_height as f32 / h as f32;
        let new_w = (w as f32 * s) as u32;
        let resized = image::imageops::resize(
            &gray,
            new_w,
            max_detection_height,
            image::imageops::FilterType::Nearest,
        );
        (resized, s)
    } else {
        (gray, 1.0)
    };

    // Otsu threshold to create binary image
    let threshold = otsu_level(&detect_gray);
    let binary: GrayImage = GrayImage::from_fn(detect_gray.width(), detect_gray.height(), |x, y| {
        if detect_gray.get_pixel(x, y)[0] < threshold {
            Luma([0u8]) // text (dark)
        } else {
            Luma([255u8]) // background (light)
        }
    });

    // Test candidate angles and find the one that maximizes projection profile variance
    let step = 0.5f32;
    let num_steps = (max_angle / step) as i32;
    let mut best_angle = 0.0f32;
    let mut best_score = f32::NEG_INFINITY;

    let (bw, bh) = (binary.width(), binary.height());

    for i in -num_steps..=num_steps {
        let angle_deg = i as f32 * step;
        let angle_rad = angle_deg.to_radians();

        // Rotate binary image
        let rotated = rotate_about_center(
            &binary,
            angle_rad,
            Interpolation::Nearest,
            Luma([255u8]), // white background fill
        );

        // Compute sum-of-squares of horizontal projection (row sums)
        let mut score = 0.0f64;
        let rh = rotated.height().min(bh);
        let rw = rotated.width().min(bw);

        for y in 0..rh {
            let mut row_sum = 0u64;
            for x in 0..rw {
                if rotated.get_pixel(x, y)[0] == 0 {
                    row_sum += 1;
                }
            }
            score += (row_sum as f64) * (row_sum as f64);
        }

        if score as f32 > best_score {
            best_score = score as f32;
            best_angle = angle_deg;
        }
    }

    // Skip rotation if angle is negligible
    if best_angle.abs() < 0.3 {
        tracing::debug!(detected_angle = best_angle, "Skew angle negligible, skipping deskew");
        return image.clone();
    }

    tracing::debug!(
        detected_angle = best_angle,
        scale = scale,
        "Detected document skew, applying correction"
    );

    // Apply rotation to the original RGB image
    let rgb = image.to_rgb8();
    let angle_rad = best_angle.to_radians();
    let rotated = rotate_about_center(
        &rgb,
        angle_rad,
        Interpolation::Bilinear,
        image::Rgb([255u8, 255, 255]),
    );

    DynamicImage::ImageRgb8(rotated)
}

/// Apply Gaussian blur for denoising
fn denoise_image(image: &DynamicImage, sigma: f32) -> DynamicImage {
    let gray = image.to_luma8();
    let blurred = gaussian_blur_f32(&gray, sigma);
    // Convert back to RGB for the pipeline
    DynamicImage::ImageLuma8(blurred).to_rgb8().into()
}

/// Enhance contrast using the specified strategy
fn enhance_contrast(image: &DynamicImage, strategy: ContrastStrategy) -> DynamicImage {
    let gray = image.to_luma8();
    let enhanced = match strategy {
        ContrastStrategy::Stretch => stretch_contrast(&gray, 0, 255, 0, 255),
        ContrastStrategy::HistogramEqualization => equalize_histogram(&gray),
    };
    // Convert back to RGB
    DynamicImage::ImageLuma8(enhanced).to_rgb8().into()
}

/// Binarize image using the specified strategy.
/// Converts to black-and-white then back to RGB for the OCR pipeline.
fn binarize_image(
    image: &DynamicImage,
    strategy: BinarizationStrategy,
    block_size: u32,
) -> DynamicImage {
    let gray = image.to_luma8();

    let binary = match strategy {
        BinarizationStrategy::Otsu => {
            let threshold = otsu_level(&gray);
            GrayImage::from_fn(gray.width(), gray.height(), |x, y| {
                if gray.get_pixel(x, y)[0] < threshold {
                    Luma([0u8])
                } else {
                    Luma([255u8])
                }
            })
        }
        BinarizationStrategy::Adaptive => {
            imageproc::contrast::adaptive_threshold(&gray, block_size)
        }
    };

    DynamicImage::ImageLuma8(binary).to_rgb8().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn make_test_image(width: u32, height: u32) -> DynamicImage {
        let img = ImageBuffer::from_fn(width, height, |x, y| {
            // Create a pattern: dark text-like strips on white
            if y % 20 < 5 && x > 10 && x < width - 10 {
                Rgb([30u8, 30, 30])
            } else {
                Rgb([240u8, 240, 240])
            }
        });
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_preprocessing_disabled_returns_clone() {
        let image = make_test_image(100, 100);
        let config = PreprocessingConfig::default();
        assert!(!config.enabled);

        let result = preprocess_image(&image, &config);
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_preprocessing_enabled_produces_valid_image() {
        let image = make_test_image(200, 200);
        let config = PreprocessingConfig {
            enabled: true,
            deskew: false, // skip deskew for speed in unit test
            denoise: true,
            denoise_sigma: 1.0,
            contrast_enhancement: true,
            contrast_strategy: ContrastStrategy::Stretch,
            binarize: false,
            ..Default::default()
        };

        let result = preprocess_image(&image, &config);
        assert_eq!(result.width(), 200);
        assert_eq!(result.height(), 200);
    }

    #[test]
    fn test_binarization_otsu() {
        let image = make_test_image(100, 100);
        let config = PreprocessingConfig {
            enabled: true,
            deskew: false,
            denoise: false,
            contrast_enhancement: false,
            binarize: true,
            binarization_strategy: BinarizationStrategy::Otsu,
            ..Default::default()
        };

        let result = preprocess_image(&image, &config);
        // After binarization, pixels should be either black or white
        let rgb = result.to_rgb8();
        for pixel in rgb.pixels() {
            assert!(
                pixel[0] == 0 || pixel[0] == 255,
                "Expected binary pixel, got {}",
                pixel[0]
            );
        }
    }

    #[test]
    fn test_binarization_adaptive() {
        let image = make_test_image(100, 100);
        let config = PreprocessingConfig {
            enabled: true,
            deskew: false,
            denoise: false,
            contrast_enhancement: false,
            binarize: true,
            binarization_strategy: BinarizationStrategy::Adaptive,
            adaptive_block_size: 15,
            ..Default::default()
        };

        let result = preprocess_image(&image, &config);
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_contrast_histogram_equalization() {
        let image = make_test_image(100, 100);
        let config = PreprocessingConfig {
            enabled: true,
            deskew: false,
            denoise: false,
            contrast_enhancement: true,
            contrast_strategy: ContrastStrategy::HistogramEqualization,
            binarize: false,
            ..Default::default()
        };

        let result = preprocess_image(&image, &config);
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_deskew_on_straight_image() {
        // A straight image should have negligible detected angle
        let image = make_test_image(300, 300);
        let result = deskew(&image, 15.0);
        assert_eq!(result.width(), 300);
        assert_eq!(result.height(), 300);
    }
}
