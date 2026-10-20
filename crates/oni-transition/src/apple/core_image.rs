//! `CIRAWFilter` set exactly as the Swift app's `RawDecode.configured`, rendered in strips into the
//! linear Rec. 2020 floats its pipeline received, so a 47 Mpx frame never sits whole in memory.

use super::AppleDecode;
use crate::measure::{BLOCK, Grid};
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::sel;
use objc2_core_foundation::{CFType, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceExtendedLinearITUR_2020};
use objc2_core_image::{
    CIContext, CIRAWFilter, kCIContextCacheIntermediates, kCIContextWorkingColorSpace, kCIContextWorkingFormat, kCIFormatRGBAf,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use std::path::Path;

/// `RawDecode.decoderVersionCeiling`: the newest decoder at or under it is pinned.
const DECODER_CEILING: i64 = 8;
/// `RawDecode.temperature`, `tint`, `colourNoiseReduction`, `decoderSharpness`, `shadowBias`.
const TEMPERATURE: f32 = 5000.0;
const TINT: f32 = 0.0;
const COLOUR_NOISE_REDUCTION: f32 = 1.0;
const SHARPNESS: f32 = 0.0;
const SHADOW_BIAS: f32 = 0.0;
/// Rows rendered at a time, a multiple of the squares' side.
const STRIP: usize = 64 * BLOCK;

pub struct CoreImage;

/// `RawDecode.pinned`: the highest version numbered at or under the ceiling.
fn pinned(raw: &CIRAWFilter) -> Option<objc2::rc::Retained<NSString>> {
    let number = |v: &NSString| v.to_string().chars().take_while(char::is_ascii_digit).collect::<String>().parse::<i64>().ok();
    let versions = unsafe { raw.supportedDecoderVersions() };
    versions.iter().filter(|v| number(v).unwrap_or(i64::MAX) <= DECODER_CEILING).max_by_key(|v| number(v).unwrap_or(0))
}

/// `RawDecode.configured`, in its order: choosing the version first restores that version's defaults.
fn configured(raw: &CIRAWFilter) {
    unsafe {
        if let Some(version) = pinned(raw) {
            raw.setDecoderVersion(&version);
        }
        raw.setBoostAmount(0.0);
        raw.setBoostShadowAmount(0.0);
        raw.setGamutMappingEnabled(false);
        raw.setNeutralTemperature(TEMPERATURE);
        raw.setNeutralTint(TINT);
        raw.setShadowBias(SHADOW_BIAS);
        raw.setColorNoiseReductionAmount(COLOUR_NOISE_REDUCTION);
        raw.setLuminanceNoiseReductionAmount(0.0);
        raw.setContrastAmount(0.0);
        raw.setDetailAmount(0.0);
        raw.setSharpnessAmount(SHARPNESS);
        raw.setMoireReductionAmount(0.0);
        raw.setLocalToneMapAmount(0.0);
        raw.setLensCorrectionEnabled(false);
        // macOS 26 and later only, as the Swift app's `#available` guard.
        if raw.respondsToSelector(sel!(setHighlightRecoveryEnabled:)) {
            raw.setHighlightRecoveryEnabled(false);
        }
        raw.setExtendedDynamicRangeAmount(0.0);
    }
}

impl AppleDecode for CoreImage {
    fn blocks(&self, raw: &Path) -> Result<Grid, String> {
        autoreleasepool(|_| decode(raw))
    }
}

fn decode(path: &Path) -> Result<Grid, String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let raw = unsafe { CIRAWFilter::filterWithImageURL(&url) }.ok_or("Core Image cannot read this RAW")?;
    configured(&raw);
    let image = unsafe { raw.outputImage() }.ok_or("Core Image decoded nothing")?;
    let extent = unsafe { image.extent() };
    let (x0, y0) = (extent.origin.x.floor(), extent.origin.y.floor());
    let (width, height) = ((extent.origin.x + extent.size.width).ceil() - x0, (extent.origin.y + extent.size.height).ceil() - y0);
    if !(width >= BLOCK as f64 && height >= BLOCK as f64 && width.is_finite() && height.is_finite()) {
        return Err("Core Image decoded an empty frame".into());
    }
    let (width, height) = (width as usize, height as usize);
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceExtendedLinearITUR_2020 })).ok_or("no linear Rec. 2020")?;
    let base: &CFType = &space;
    let space_object: &AnyObject = base.as_ref();
    let format = NSNumber::numberWithInt(unsafe { kCIFormatRGBAf });
    let no = NSNumber::numberWithBool(false);
    let options = NSDictionary::from_slices(
        &[unsafe { kCIContextWorkingColorSpace }, unsafe { kCIContextWorkingFormat }, unsafe { kCIContextCacheIntermediates }],
        &[space_object, format.as_ref(), no.as_ref()],
    );
    let context = unsafe { CIContext::contextWithOptions(Some(&options)) };
    let (gw, gh) = (width / BLOCK, height / BLOCK);
    let mut means = vec![[0f32; 3]; gw * gh];
    let mut strip = vec![0f32; width * STRIP * 4];
    for top in (0..gh * BLOCK).step_by(STRIP) {
        let rows = STRIP.min(gh * BLOCK - top);
        // Core Image's y rises from the bottom; the bitmap's first row is the strip's top.
        let bounds = CGRect::new(CGPoint::new(x0, y0 + (height - top - rows) as f64), CGSize::new(width as f64, rows as f64));
        let data = std::ptr::NonNull::new(strip.as_mut_ptr().cast()).ok_or("no strip")?;
        unsafe {
            context.render_toBitmap_rowBytes_bounds_format_colorSpace(&image, data, (width * 16) as isize, bounds, kCIFormatRGBAf, Some(&space));
        }
        for by in 0..rows / BLOCK {
            for bx in 0..gw {
                let mut s = [0f64; 3];
                for y in by * BLOCK..(by + 1) * BLOCK {
                    for x in bx * BLOCK..(bx + 1) * BLOCK {
                        (0..3).for_each(|c| s[c] += f64::from(strip[(y * width + x) * 4 + c]));
                    }
                }
                means[(top / BLOCK + by) * gw + bx] = s.map(|v| (v / (BLOCK * BLOCK) as f64) as f32);
            }
        }
    }
    Ok(Grid { width: gw, height: gh, means })
}
