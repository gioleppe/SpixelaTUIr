# Real-Time Webcam Input — Design Specification

## 1. Overview

This document specifies the design for **real-time webcam capture** in Spix.
The feature allows users to feed a live video stream from a connected camera
into the effect pipeline, rendering the glitched output directly to the
terminal at interactive frame rates. This transforms Spix from a static image
editor into a live visual performance tool — ideal for VJing, streaming
overlays, and creative experimentation.

---

## 2. Motivation

| Use Case | Description |
|----------|-------------|
| **Live glitch performance** | Apply real-time pixel sort, CRT, and color effects to a webcam feed for live visuals |
| **Creative exploration** | Quickly preview how effects look on a human face or moving scene |
| **Stream overlays** | Use Spix as a glitch filter for OBS virtual camera input |
| **Photobooth mode** | Capture glitched stills from the live feed with a single keypress |

---

## 3. Technology Choice: `nokhwa`

| Criterion | `nokhwa` | Alternative (`v4l2`, `opencv`) |
|-----------|----------|--------------------------------|
| Cross-platform | ✅ Linux (V4L2), macOS (AVFoundation), Windows (Media Foundation) | `v4l2` Linux-only; `opencv` heavy |
| Rust-native | ✅ Pure Rust crate | `opencv` requires C++ bindings |
| Frame format | RGBA/RGB/YUYV with auto-conversion | Manual format handling |
| Camera enumeration | ✅ List available devices | Manual |
| Async capture | ✅ Callback-based frame delivery | Blocking read |
| Binary size | ~1–2 MB | `opencv` adds 50+ MB |

**Decision:** Use `nokhwa` with platform-specific backends enabled via
Cargo features.

---

## 4. Architecture

### 4.1 Thread Model

```
┌──────────────────────────────────────────────────────────┐
│                  MAIN THREAD (UI)                        │
│  - Event loop (keyboard, terminal resize)                │
│  - Displays latest processed frame from preview_buffer   │
│  - Sends WorkerCommand::ProcessWebcamFrame               │
└──────────┬───────────────────────────┬───────────────────┘
           │                           │
    WorkerCommand               WorkerResponse
           │                           │
           ▼                           │
┌──────────────────────────────┐       │
│     WORKER THREAD            │       │
│  - Applies pipeline to frame │───────┘
│  - Returns ProcessedFrame    │
└──────────────────────────────┘
           ▲
           │ raw frame (mpsc)
           │
┌──────────────────────────────┐
│     CAPTURE THREAD (new)     │
│  - nokhwa camera loop        │
│  - Captures frames at Nfps   │
│  - Converts to DynamicImage  │
│  - Sends to UI via channel   │
└──────────────────────────────┘
```

A dedicated **Capture Thread** is introduced to decouple camera I/O from both
the UI and the processing pipeline. The capture thread runs a tight loop:

1. Grab frame from camera → `nokhwa::Buffer`
2. Convert to `image::DynamicImage` (RGBA8)
3. Downscale to proxy resolution
4. Send via `mpsc::Sender<DynamicImage>` to the UI thread

The UI thread receives the latest frame, replaces `proxy_asset`, and dispatches
a `WorkerCommand::Process` to the worker thread. If a previous frame is still
being processed, the new frame is **dropped** (latest-frame semantics, no
queue buildup).

### 4.2 Frame Rate Governance

```
Target pipeline FPS = min(camera_fps, terminal_refresh_rate, processing_capacity)
```

The capture thread uses a frame rate limiter (e.g., 15 fps default) to avoid
overwhelming the processing pipeline. The UI thread maintains a `last_frame`
timestamp and skips dispatch if the worker is still busy (checked via a
`worker_busy: AtomicBool` flag).

---

## 5. Data Structures

### 5.1 WebcamState

```rust
#[derive(Debug)]
pub struct WebcamState {
    /// Whether webcam mode is currently active.
    pub active: bool,
    /// Currently selected camera device index.
    pub device_index: usize,
    /// Available camera devices (enumerated at startup or on request).
    pub devices: Vec<CameraDeviceInfo>,
    /// Capture resolution (width, height).
    pub capture_resolution: (u32, u32),
    /// Target capture frame rate.
    pub target_fps: u32,
    /// Actual measured frame rate (rolling average).
    pub actual_fps: f32,
    /// Channel receiver for incoming frames from the capture thread.
    pub frame_rx: Option<mpsc::Receiver<DynamicImage>>,
    /// Handle to the capture thread (for shutdown).
    pub capture_handle: Option<std::thread::JoinHandle<()>>,
    /// Signal to stop the capture thread.
    pub stop_signal: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
pub struct CameraDeviceInfo {
    pub index: usize,
    pub name: String,
    pub supported_resolutions: Vec<(u32, u32)>,
    pub supported_fps: Vec<u32>,
}
```

### 5.2 AppState Changes

```rust
// In AppState:
pub webcam: WebcamState,
```

### 5.3 InputMode Addition

```rust
InputMode::WebcamDevicePicker,  // Camera selection dialog
```

---

## 6. Capture Thread Implementation

```rust
fn webcam_capture_loop(
    device_index: usize,
    resolution: (u32, u32),
    target_fps: u32,
    proxy_long_edge: u32,
    frame_tx: mpsc::Sender<DynamicImage>,
    stop_signal: Arc<AtomicBool>,
) {
    let camera = Camera::new(
        CameraIndex::Index(device_index),
        RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::Closest(CameraFormat::new(
                Resolution::new(resolution.0, resolution.1),
                FrameFormat::RawRgb,
                target_fps,
            ))
        ),
    ).expect("Failed to open camera");

    camera.open_stream().expect("Failed to start stream");

    let frame_interval = Duration::from_millis(1000 / target_fps as u64);

    while !stop_signal.load(Ordering::Relaxed) {
        let start = Instant::now();

        if let Ok(buffer) = camera.frame() {
            if let Ok(image) = buffer.decode_image::<RgbFormat>() {
                let dynamic = DynamicImage::ImageRgb8(image);
                let proxy = downscale_to_proxy(&dynamic, proxy_long_edge);

                // Non-blocking send: if the UI hasn't consumed the last frame, drop this one
                let _ = frame_tx.try_send(proxy);
            }
        }

        let elapsed = start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    camera.stop_stream().ok();
}
```

---

## 7. TUI Integration

### 7.1 Status Bar

When webcam mode is active, the status bar shows:

```
Webcam | HD Pro Webcam C920 [640×480] 15fps              Pipeline: 12ms ⚡
```

- Camera name and resolution
- Actual capture fps
- Pipeline processing latency per frame

### 7.2 Canvas

The canvas continuously updates with the latest processed frame. The title
changes from `"Preview"` to `"📷 Live"` to indicate webcam mode.

### 7.3 Device Picker Dialog

```
┌──────────────────────────────────────────┐
│ Select Camera                            │
│                                          │
│ ► HD Pro Webcam C920                     │
│   FaceTime HD Camera                     │
│   OBS Virtual Camera                     │
│                                          │
│ Resolution: 640×480  │  FPS: 15          │
│                                          │
│ Enter: Select  │  ←/→: Resolution  │  Esc│
└──────────────────────────────────────────┘
```

---

## 8. Keyboard Shortcuts

### 8.1 Normal Mode

| Key | Action |
|-----|--------|
| `W` | Toggle webcam mode on/off |
| `Shift+W` | Open camera device picker |

### 8.2 Webcam Mode Active

| Key | Action |
|-----|--------|
| `c` | Capture current frame as a still (saves to `source_asset`, exits webcam mode) |
| `+` / `-` | Increase / decrease target fps (5, 10, 15, 24, 30) |
| `[` / `]` | Decrease / increase capture resolution |
| `W` | Stop webcam mode, return to last loaded image |
| All normal effect shortcuts | Continue working (add effects, adjust params, etc.) |

### 8.3 During Webcam Capture

All existing pipeline editing shortcuts remain active. The user can:

- Add/remove effects while watching the live result
- Adjust effect parameters in real time
- Toggle effects on/off for immediate A/B comparison
- Randomise the pipeline for happy accidents

---

## 9. Still Capture from Webcam

Pressing `c` during webcam mode:

1. Captures the current raw (unprocessed) frame at full capture resolution.
2. Stores it as `source_asset` (replacing any previously loaded image).
3. Generates a `proxy_asset` at the current proxy resolution.
4. Exits webcam mode.
5. The captured image is now a normal Spix editing session.
6. Status bar: `"📷 Captured frame — webcam stopped"`.

This allows a workflow of: browse the live feed → find an interesting moment →
capture → fine-tune effects on the still.

---

## 10. Performance Considerations

### 10.1 Frame Dropping

The capture thread uses a **bounded channel** (`mpsc::sync_channel(1)`) or
`try_send` to implement latest-frame semantics. If the worker is still
processing the previous frame, the new frame is dropped. This prevents
unbounded memory growth and ensures the preview always shows the most recent
frame.

### 10.2 Proxy Resolution

Webcam frames are immediately downscaled to the proxy resolution (same as
image mode) before being sent to the UI thread. This ensures:

- Channel transfer is fast (small images)
- Pipeline processing matches image-mode performance
- The Sixel rendering pipeline is not overwhelmed

### 10.3 Pipeline Latency Budget

| Component | Target | Actual (estimate) |
|-----------|--------|-------------------|
| Camera capture | 33 ms (30 fps) | Camera-dependent |
| Proxy downscale | <5 ms | 2–3 ms for 1080p → 512px |
| Channel transfer | <1 ms | ~0 ms (pointer move) |
| Pipeline processing | <50 ms | Depends on effects (proxy is small) |
| Sixel rendering | <10 ms | 5–8 ms for 512px proxy |
| **Total** | **<100 ms** | **~50 ms → 20 fps effective** |

---

## 11. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| No camera detected | `W` key shows: `"No camera detected"` in status bar |
| Camera in use by another app | Status bar: `"Camera busy — close other apps"` |
| Camera disconnected during capture | Capture thread detects error; auto-stops webcam mode; status bar warning |
| Frame decode failure | Skip frame silently; increment dropped frame counter |
| Permission denied (macOS) | Status bar: `"Camera access denied — check System Preferences"` |

---

## 12. Cargo.toml Changes

```toml
[features]
default = ["webcam"]
webcam = ["dep:nokhwa"]

# Platform-specific camera backend features.
# Only the platform-specific dependency block is used at build time — they
# are mutually exclusive via cfg gates, not additive.

[target.'cfg(target_os = "linux")'.dependencies]
nokhwa = { version = "0.10", optional = true, features = ["input-v4l"] }

[target.'cfg(target_os = "macos")'.dependencies]
nokhwa = { version = "0.10", optional = true, features = ["input-avfoundation"] }

[target.'cfg(target_os = "windows")'.dependencies]
nokhwa = { version = "0.10", optional = true, features = ["input-msmf"] }
```

Feature-gated to keep the binary small for users who don't have a camera:

```sh
cargo build --release --no-default-features   # no webcam support
```

---

## 13. File Changes Summary

| File | Change |
|------|--------|
| `Cargo.toml` | Add `nokhwa` dependency with platform features; `webcam` feature gate |
| `src/app/webcam.rs` *(new)* | `WebcamState`, `CameraDeviceInfo`, capture thread spawn/stop |
| `src/app/state.rs` | Add `webcam: WebcamState` field; integrate frame receiving into event loop |
| `src/app/handlers.rs` | `W`/`Shift+W`/`c` key handlers; webcam-aware event loop branch |
| `src/app/dialogs.rs` | `WebcamDevicePickerDialog` |
| `src/app/mod.rs` | Re-export webcam module; integrate capture frame polling into `run()` |
| `src/ui/canvas.rs` | `"📷 Live"` title when webcam active |
| `src/ui/widgets.rs` | Webcam info in status bar (device name, resolution, fps, latency) |
| `README.md` | Document webcam feature, shortcuts, performance tips |

---

## 14. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Camera enumeration** | `nokhwa` integration, device discovery, `CameraDeviceInfo` | Camera list available |
| **2 — Capture thread** | Spawn/stop capture thread, frame channel, proxy downscale | Raw frames reaching UI thread |
| **3 — Live preview** | Wire captured frames into existing preview pipeline | Live glitched webcam in terminal |
| **4 — Device picker UI** | Dialog for selecting camera, resolution, fps | User can choose camera |
| **5 — Still capture** | `c` key captures frame, exits webcam, stores as source | Capture-to-edit workflow |
| **6 — Performance tuning** | Frame dropping, bounded channel, latency measurement | Smooth 15+ fps live preview |
| **7 — UX polish** | Status bar indicators, error messages, documentation | Feature complete |

---

## 15. Open Questions

1. **Audio capture:** Should Spix support audio input alongside video for
   audio-reactive effects (e.g., glitch intensity tied to audio levels)?
   Recommendation: defer to a separate spec — audio is a distinct feature.

2. **Virtual camera output:** Should Spix be able to act as a virtual camera
   (output the glitched feed as a V4L2/OBS source)? Recommendation: yes,
   but defer to Phase 8 — requires `v4l2loopback` on Linux.

3. **Recording mode:** Should there be a "record" button that captures a
   sequence of live frames for GIF/video export? Recommendation: yes —
   integrate with the existing Animation Timeline by capturing frames into
   the animation buffer.

4. **Multiple cameras:** Should Spix support multiple simultaneous camera
   inputs (e.g., for picture-in-picture or layer-per-camera)?
   Recommendation: defer — single camera covers the primary use case.

5. **Camera settings:** Should Spix expose camera controls (exposure,
   white balance, focus) via the TUI? `nokhwa` supports these.
   Recommendation: yes, in a Phase 8 "Camera Settings" dialog.
