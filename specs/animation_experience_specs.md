# Animation Panel — UX & Experience Specification

## 1. Overview

This document defines the user experience, TUI layout, interaction model, and
data structures for the **Animation Panel** feature in SpixelaTUIr. The goal is
to let users build multi-frame animations where each frame is a variation of
one or more effect parameters, then preview and export the result as an
animated GIF or WebP.

The design deliberately fits the existing multi-threaded,
keyboard-first TUI philosophy: the UI thread stays responsive, the worker
thread handles frame rendering, and a new export thread stitches the final
animation.

---

## 2. Core Mental Model

> **An animation is a sequence of pipeline snapshots.**

Each **frame** stores a complete `Pipeline` state. Frames are captured
manually ("frame-by-frame" mode) or generated automatically ("sweep" mode).
During playback the canvas cycles through the pre-rendered frame images at a
user-defined frame rate.

```
Source image
     │
     ▼
 Frame 1: Pipeline A ──► rendered preview F1
 Frame 2: Pipeline B ──► rendered preview F2
 Frame 3: Pipeline C ──► rendered preview F3
     │
     ▼
  Export (GIF / WebP)
```

---

## 3. Two Authoring Workflows

### 3.1 Frame-by-Frame Capture (manual)

The user composes a pipeline in the existing Effects panel, tweaks parameters
until the frame looks right, then presses **`c`** (capture) while the
Animation Panel is focused. The current pipeline + preview buffer is added as
a new frame at the end of the timeline (or inserted after the selected frame).

**Typical session:**

1. Load image (`o`).
2. Open animation panel (`Ctrl+N`).
3. Build effect chain (Effects panel).
4. Focus animation panel, press `c` → Frame 1 captured.
5. Return to effects panel, adjust HueShift from 30° to 90°.
6. Focus animation panel, press `c` → Frame 2 captured.
7. Repeat for as many frames as desired.
8. Press `Space` to preview the animation in the canvas.
9. Press `Ctrl+E` to export GIF/WebP.

### 3.2 Parameter Sweep (automatic)

The user selects one effect parameter and defines a **start value**, **end
value**, and **frame count**. The system auto-generates all intermediate frames
by linearly interpolating the parameter value across the specified range. All
other parameters stay constant.

**Typical session:**

1. Build a pipeline (e.g., HueShift + Scanlines).
2. Open the sweep dialog: focus animation panel, press `s`.
3. Select the parameter to sweep (e.g., HueShift → degrees).
4. Enter start: `0`, end: `360`, frames: `24`.
5. Confirm → 24 frames generated and rendered by the worker.
6. Preview + export.

Sweep mode is ideal for smooth loops (e.g., full hue rotation 0→360°,
Pixelate block size 2→32, RowJitter 0→1→0 for a trembling animation).

---

## 4. TUI Layout

### 4.1 Normal mode (animation panel hidden)

The existing 4-panel layout is unchanged:

```
┌──────────────────────────────────────────────────────────┐ ← 1 line
│ Status Bar                                               │
├──────────────────────────────────────┬───────────────────┤
│                                      │                   │
│  Canvas                              │  Effects Panel    │
│  (flexible height)                   │  (26 cols fixed)  │
│                                      │                   │
├──────────────────────────────────────┴───────────────────┤ ← 3 lines
│ Controls Hint                                            │
└──────────────────────────────────────────────────────────┘
```

### 4.2 Animation mode active (`Ctrl+N` toggle)

The Animation Panel replaces the Controls Hint row and expands to **7 lines**
of height. The canvas loses those 7 lines.

```
┌──────────────────────────────────────────────────────────┐ ← 1 line
│ Status Bar                                               │
├──────────────────────────────────────┬───────────────────┤
│                                      │                   │
│  Canvas (reduced height)             │  Effects Panel    │
│                                      │  (26 cols fixed)  │
│                                      │                   │
├──────────────────────────────────────┴───────────────────┤ ← 7 lines
│ Animation Panel                                          │
│                                                          │
│  ▶ [F01*][F02 ][F03 ][F04 ][F05 ][F06 ]  6 frames       │
│    100ms  100ms 100ms 100ms 100ms  100ms   fps: 10       │
│  Loop: yes   Status: ready                               │
│                                                          │
│  c:capture  s:sweep  Del:remove  Space:play  Ctrl+E:export │
└──────────────────────────────────────────────────────────┘
```

**Panel anatomy (7 lines):**

| Line | Content |
|------|---------|
| 1 | Title border: `Animation (N frames)` |
| 2 | Frame strip — horizontally scrollable `[F01*][F02 ]…` |
| 3 | Per-frame duration row (ms value under each slot) |
| 4 | Global controls: Loop toggle, playback status, fps |
| 5 | Playback progress bar during playback (hidden at rest) |
| 6 | (reserved / empty) |
| 7 | Context-sensitive key hint bar |

### 4.3 Frame strip notation

Each frame slot is 6 characters wide: `[F01*]`

- `F01` — 1-based index (zero-padded to 2 digits)
- `*` — currently selected frame (space otherwise)
- Border: cyan when selected, white otherwise
- Slots scroll horizontally; up to ⌊(panel width − 4) / 6⌋ slots visible at once
- A `◄` or `►` indicator at the left/right edge indicates off-screen frames

**Visual example (terminal 80 cols, panel 76 cols inner):**

```
  [F01 ][F02 ][F03*][F04 ][F05 ][F06 ][F07 ][F08 ][F09 ][F10 ][F11 ] ►
   100ms 100ms  80ms 100ms 100ms 100ms  60ms 100ms 100ms 100ms 100ms
```

### 4.4 Playback in canvas

During playback the canvas renders each frame image in sequence using the
existing `image_protocol` mechanism. The canvas title changes to
`"Preview (F03/10)"` to show current position. The Effects Panel is grayed
out (border dark gray, no interaction) while playback is active.

---

## 5. Keyboard Shortcuts

All animation shortcuts are active only when the Animation Panel is focused
**or** when the global animation input mode is active. No existing shortcuts
are overridden.

### 5.1 Global (any mode, except text-input dialogs)

| Key | Action |
|-----|--------|
| `Ctrl+N` | Toggle Animation Panel open / closed |

### 5.2 Animation Panel focused

| Key | Action |
|-----|--------|
| `←` / `h` | Select previous frame |
| `→` / `l` | Select next frame |
| `c` | Capture current pipeline as a new frame (appended after selection) |
| `s` | Open Parameter Sweep dialog |
| `d` / `Delete` | Remove selected frame from timeline |
| `Space` | Play / pause animation preview |
| `Enter` | Load selected frame's pipeline back into the Effects panel for editing |
| `f` | Edit frame duration (ms) — opens inline numeric input |
| `F` | Set all frame durations to same value — opens inline numeric input |
| `L` | Toggle loop mode (on / off) |
| `+` / `-` | Increase / decrease global frame rate (affects display fps) |
| `Ctrl+E` | Open animation export dialog |
| `Esc` | Return focus to Effects panel |
| `Tab` | Cycle focus: Canvas → Effects → Animation (extends existing `Tab` cycle) |

### 5.3 During playback

| Key | Action |
|-----|--------|
| `Space` | Pause |
| `Esc` | Stop playback, return to first frame |
| Any other key | Stop playback (preserves current frame position) |

### 5.4 Parameter Sweep dialog

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate fields |
| `Tab` | Next field |
| Character input | Edit numeric field |
| `Enter` | Confirm sweep (triggers batch worker render) |
| `Esc` | Cancel |

Fields:

1. **Effect** — dropdown: select an effect from the current pipeline
2. **Parameter** — dropdown: select a numeric parameter of that effect
3. **Start** — numeric (float or int, pre-filled with current value)
4. **End** — numeric
5. **Frames** — integer (2–240)
6. **Easing** — dropdown: Linear / Ease-In / Ease-Out / Ease-In-Out / Ping-Pong

### 5.5 Animation Export dialog

Extends the existing export dialog with animation-specific fields:

| Field | Options / Notes |
|-------|----------------|
| Directory | Same as still export |
| Filename | Base name (no extension) |
| Format | `GIF` / `WebP (animated)` |
| Quality | 1–100 (WebP lossy) / ignored for GIF |
| Dithering | Floyd-Steinberg / None (GIF only) |
| Resolution | Proxy / Full source |
| Loop | Yes / No |

---

## 6. Data Structures

```rust
/// A single animation frame: a frozen pipeline snapshot + metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationFrame {
    /// The full pipeline state for this frame.
    pub pipeline: Pipeline,
    /// How long this frame is displayed during playback, in milliseconds.
    pub duration_ms: u32,
    /// Optional label shown in the timeline strip.
    pub label: Option<String>,
}

/// The full animation timeline held in AppState.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnimationTimeline {
    pub frames: Vec<AnimationFrame>,
    /// Index of the currently selected frame in the timeline strip.
    pub selected: usize,
    /// Global frames-per-second used for uniform-duration playback.
    pub fps: u8,
    /// Whether export and playback should loop.
    pub loop_mode: bool,
}

impl AnimationTimeline {
    /// Returns the effective display duration for a frame at `idx` in
    /// milliseconds. Uses the frame's own `duration_ms` if it is non-zero;
    /// otherwise falls back to `1000 / fps` (the globally uniform interval).
    /// Saturates to 1 ms if `fps` is 0 to avoid a divide-by-zero panic.
    pub fn frame_duration_ms(&self, idx: usize) -> u32 { ... }
}
```

### 6.1 AppState additions

```rust
// Inside AppState:
pub animation: AnimationTimeline,
pub animation_panel_open: bool,
pub animation_playback: AnimationPlaybackState,
/// Pre-rendered proxy images for each animation frame, indexed by frame
/// position. `None` means the frame has not yet been rendered or has been
/// marked dirty after a pipeline edit. Populated by the worker thread in
/// response to `WorkerCommand::RenderAnimationFrame` /
/// `WorkerCommand::RenderSweepBatch`.
pub animation_rendered_frames: Vec<Option<DynamicImage>>,
```

```rust
#[derive(Debug, Default)]
pub enum AnimationPlaybackState {
    #[default]
    Stopped,
    Playing {
        current_frame: usize,
        /// Instant the current frame started displaying.
        frame_started: std::time::Instant,
    },
    Paused { current_frame: usize },
}
```

### 6.2 Worker commands

```rust
// Additional WorkerCommand variants:
WorkerCommand::RenderAnimationFrame {
    image: DynamicImage,      // proxy or full source
    pipeline: Pipeline,
    frame_idx: usize,
    total_frames: usize,
    response_tx: Sender<WorkerResponse>,
},
WorkerCommand::RenderSweepBatch {
    image: DynamicImage,
    frames: Vec<Pipeline>,    // pre-interpolated pipelines from sweep dialog
    response_tx: Sender<WorkerResponse>,
},

// Additional WorkerResponse variants:
WorkerResponse::AnimationFrameReady {
    frame_idx: usize,
    image: DynamicImage,
},
WorkerResponse::SweepBatchReady {
    images: Vec<DynamicImage>,
},
WorkerResponse::AnimationExported(PathBuf),
```

### 6.3 InputMode additions

```rust
// New variants added to InputMode:
InputMode::AnimationPanel,
InputMode::AnimationSweepDialog,
InputMode::AnimationExportDialog,
InputMode::AnimationFrameDurationInput,
InputMode::AnimationPlayback,
```

---

## 7. Playback Engine

Playback runs entirely on the main (UI) thread using the existing 16 ms event
loop tick. No additional threads are required for playback.

**Tick logic (inside the event loop):**

```
if matches!(app.animation_playback, Playing { .. }) {
    let elapsed = frame_started.elapsed().as_millis();
    let duration = animation.frame_duration_ms(current_frame);

    if elapsed >= duration {
        let next = advance_frame(current_frame, animation.frames.len(), animation.loop_mode);
        if let Some(idx) = next {
            app.set_preview(pre_rendered[idx].clone());
            app.animation_playback = Playing { current_frame: idx, frame_started: Instant::now() };
        } else {
            app.animation_playback = Stopped;
        }
    }
}
```

Pre-rendered frames (small `DynamicImage` values at proxy resolution) are
cached in `AppState::animation_rendered_frames: Vec<Option<DynamicImage>>` so
playback only calls `set_preview()` — no worker dispatch on each tick.

A frame is marked "dirty" (its slot shows `[F03?]`) when its pipeline has been
edited since the last render. Pressing `Space` to play re-renders dirty frames
via the worker before starting.

---

## 8. Sweep Interpolation

All interpolation is done in the UI thread before dispatching to the worker
batch. Only numeric parameters (`f32` / `i32`) are interpolated. Boolean and
preset-enum parameters are snapped at the midpoint of the sweep range.

```
Linear:       t = i / (n - 1)
Ease-In:      t = t²
Ease-Out:     t = 1 - (1-t)²
Ease-In-Out:  t = t < 0.5  ? 2t²  :  1 - (-2t + 2)² / 2
Ping-Pong:    frames 0..n/2 go start→end, frames n/2..n go end→start
              (generates a seamless loop automatically)
```

`Ping-Pong` doubles the frame count internally — entering `n=12` produces 24
frames that loop smoothly.

---

## 9. Animation Export

Export is handled by a dedicated thread spawned at export time (matching the
existing still-image export pattern).

### 9.1 GIF export

- Uses `image::codecs::gif::GifEncoder`
- Each frame: `image::Frame::from_parts(rgba_img, 0, 0, delay)`
- `delay` comes from `AnimationFrame::duration_ms` (centiseconds for GIF)
- Floyd-Steinberg dithering available via `image::imageops::dither`
- Auto-incrementing filename (`output_anim_1.gif`, `output_anim_2.gif`, …)

### 9.2 Animated WebP export

- Uses `image::codecs::webp::WebPEncoder` (available since `image` 0.25)
- Lossy or lossless controlled by quality field
- Preserves per-frame timing

### 9.3 Resolution choice

| Choice | Source image |
|--------|-------------|
| Proxy | Uses the existing `proxy_asset` — fast, small file, good for quick sharing |
| Full | Re-renders every frame from `source_asset` on a dedicated export thread; slow but high-quality |

Status bar shows progress: `"Exporting animation… 12/24 frames"`.

---

## 10. Pipeline Persistence

Animation timelines are saved as a superset of the existing pipeline JSON:

```json
{
  "animation": {
    "fps": 10,
    "loop_mode": true,
    "frames": [
      {
        "duration_ms": 100,
        "label": "Frame 1",
        "pipeline": { "effects": [ ... ] }
      },
      {
        "duration_ms": 80,
        "label": "Glitch peak",
        "pipeline": { "effects": [ ... ] }
      }
    ]
  }
}
```

`Ctrl+S` saves the full animation timeline alongside the pipeline. Loading a
file that contains an `animation` key restores the timeline automatically.
Loading a plain pipeline JSON (no `animation` key) behaves exactly as today.

---

## 11. Status Bar Integration

The status bar gains a small animation indicator on the right side when the
animation panel is open:

```
Ready | /path/to/image.png [512px]                    ANIM 6F 10fps ▶
```

- `ANIM` — animation mode active
- `6F` — frame count
- `10fps` — current fps setting
- `▶` — playing; `■` — stopped; `⏸` — paused

---

## 12. Error Handling & Edge Cases

| Situation | Behaviour |
|-----------|-----------|
| Capture with no image loaded | Status bar: `"No image loaded — open an image first"` |
| Capture with empty pipeline | Allowed — frame stores empty pipeline (shows original) |
| Delete last frame | Timeline becomes empty; playback state reset to `Stopped` |
| Sweep on non-numeric param | Sweep dialog disables that parameter in the dropdown |
| Export with 0 or 1 frame | Export button disabled; hint: `"Add at least 2 frames"` |
| Worker still rendering sweep | Export disabled; status: `"Rendering frames… N/M"` |
| Unsaved animation on quit | Existing `ConfirmQuit` modal also fires for unsaved animation |

---

## 13. Proposed Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Data model** | `AnimationTimeline`, `AnimationFrame`, `AnimationPlaybackState` structs; `InputMode` new variants | Compiling data model, unit tests |
| **2 — Panel UI** | `AnimationPanel` layout, frame strip widget, key hint bar, `Tab` cycle extension | Visual panel (no capture yet) |
| **3 — Capture & edit** | `c` key captures frame; `Enter` loads frame back to pipeline; `f`/`F` duration edit; `d` delete | Frame-by-frame authoring complete |
| **4 — Playback** | `AnimationPlaybackState` tick logic, `set_preview` cycle, status bar indicator | In-app looping preview |
| **5 — Sweep dialog** | Dialog UI + interpolation math + `RenderSweepBatch` worker command | Parameter sweep complete |
| **6 — Export** | Animation export dialog, GIF & WebP encoding, progress reporting | Full end-to-end export |
| **7 — Persistence** | Save/load animation timeline JSON; backward-compat with plain pipeline files | Reloadable animation files |

---

## 14. Open Questions

1. **Thumbnail previews in frame strip** — Should each `[F01]` slot show a
   tiny Sixel thumbnail (e.g. 12×8 px Sixel block)? This would require
   pre-rendering miniature previews on the worker thread. Adds richness but
   complicates the layout. Could be a Phase 8 enhancement.

2. **Frame reordering** — Should `K`/`J` (shift) work in the Animation Panel
   to reorder frames, mirroring the effect-reorder UX? Recommended: yes,
   consistent with existing drag metaphor.

3. **Multiple parameter sweep** — Should the sweep dialog support sweeping
   two parameters simultaneously (e.g., HueShift + Noise intensity)? Could be
   deferred to a later phase.

4. **Undo/redo scope** — Should individual frame captures be undoable via the
   existing `Ctrl+Z` stack? Recommended: yes — each capture pushes to the same
   undo ring.

5. **Animation-aware randomize** — Should pressing `r` randomize only the
   currently selected frame's pipeline, or all frames? Likely safest to scope
   to the current frame only when the animation panel is active.
