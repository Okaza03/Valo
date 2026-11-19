//! Hardware video decoding. The decoder gets a GPU device before it opens;
//! FFmpeg's default `get_format` then decodes on it when the GPU supports the
//! stream, and falls back to software by itself when it doesn't (10-bit H.264,
//! say) or when the hardware decoder fails to start. Decoded frames live in
//! GPU memory and are copied back with [`download`] before the RGBA scaler.
use std::ptr;

use ffmpeg::sys::{self, AVHWDeviceType as Dev};
use ffmpeg::{codec, frame};
use ffmpeg_next as ffmpeg;

/// The devices to try, best first. They must be built into FFmpeg: Linux
/// and macOS turn them on in Cargo.toml; Windows' prebuilt FFmpeg has them.
#[cfg(target_os = "linux")]
const DEVICES: &[Dev] = &[Dev::AV_HWDEVICE_TYPE_VULKAN];
#[cfg(target_os = "macos")]
const DEVICES: &[Dev] = &[Dev::AV_HWDEVICE_TYPE_VIDEOTOOLBOX];
#[cfg(windows)]
const DEVICES: &[Dev] = &[Dev::AV_HWDEVICE_TYPE_D3D11VA, Dev::AV_HWDEVICE_TYPE_VULKAN];
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const DEVICES: &[Dev] = &[];

fn name(d: Dev) -> &'static str {
    match d {
        Dev::AV_HWDEVICE_TYPE_VULKAN => "Vulkan",
        Dev::AV_HWDEVICE_TYPE_VIDEOTOOLBOX => "VideoToolbox",
        Dev::AV_HWDEVICE_TYPE_D3D11VA => "Direct3D 11",
        _ => "GPU",
    }
}

/// Whether the decoder for `codec` can output frames on a `d` device.
///
/// # Safety
/// `codec` must be a valid decoder from `avcodec_find_decoder`.
unsafe fn supports(codec: *const sys::AVCodec, d: Dev) -> bool {
    for i in 0.. {
        let cfg = sys::avcodec_get_hw_config(codec, i);
        if cfg.is_null() {
            return false;
        }
        let by_device = (*cfg).methods & sys::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0;
        if by_device && (*cfg).device_type == d {
            return true;
        }
    }
    false
}

/// Give a not-yet-opened decoder the first GPU device that can decode its
/// codec. Returns the device's name, or `None` to decode in software.
pub fn attach(ctx: &mut codec::context::Context) -> Option<&'static str> {
    // SAFETY: ctx wraps a live, unopened AVCodecContext; the device reference
    // av_hwdevice_ctx_create returns is handed to it, which unrefs it when freed.
    unsafe {
        let c = ctx.as_mut_ptr();
        let codec = sys::avcodec_find_decoder((*c).codec_id);
        if codec.is_null() {
            return None;
        }
        for &d in DEVICES {
            if !supports(codec, d) {
                continue;
            }
            let mut dev = ptr::null_mut();
            if sys::av_hwdevice_ctx_create(&mut dev, d, ptr::null(), ptr::null_mut(), 0) < 0 {
                continue; // no such GPU or driver here
            }
            (*c).hw_device_ctx = dev;
            return Some(name(d));
        }
    }
    None
}

/// Whether `f` is in GPU memory.
pub fn on_gpu(f: &frame::Video) -> bool {
    // SAFETY: f wraps a live AVFrame; hw_frames_ctx is set only for GPU frames
    unsafe { !(*f.as_ptr()).hw_frames_ctx.is_null() }
}

/// Copy a GPU frame into system memory (NV12, P010, … as the GPU gives it).
pub fn download(f: &frame::Video) -> Result<frame::Video, ffmpeg::Error> {
    let mut out = frame::Video::empty();
    // SAFETY: both wrap live AVFrames; `out` is empty, so FFmpeg allocates it
    // in the device's preferred download format.
    let r = unsafe { sys::av_hwframe_transfer_data(out.as_mut_ptr(), f.as_ptr(), 0) };
    if r < 0 {
        return Err(ffmpeg::Error::from(r));
    }
    Ok(out)
}
