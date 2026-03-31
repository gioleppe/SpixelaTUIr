# Advanced Keyframe Animation with Easing — Design Specification

## 1. Overview

This document specifies the design for an **advanced keyframe animation
system** that extends Spix's existing Animation Panel (see
`animation_experience_specs.md`). Instead of capturing individual frames
manually or sweeping a single parameter linearly, users define **keyframes**
at specific points on a timeline. Each keyframe stores values for one or
more effect parameters. The system interpolates between keyframes using
configurable easing curves to generate smooth, complex multi-parameter
animations.

This elevates Spix from a frame-by-frame capture tool to a full
motion-graphics animation engine for glitch art.

---

## 2. Motivation

| Limitation | Desired Outcome |
|------------|-----------------|
| Current sweep mode handles only one parameter at a time | Animate HueShift + Noise intensity + Pixelate block size simultaneously |
| Linear interpolation only | Ease-in, ease-out, bounce, elastic curves for professional motion |
| Manual frame-by-frame capture is tedious for complex animations | Define 3–5 keyframes and let the system generate 120+ frames |
| No timeline scrubbing | Jump to any point in the animation, preview, adjust |
| No looping control beyond simple on/off | Ping-pong, reverse, hold-last-frame modes |

---

## 3. Core Mental Model

> **Keyframes are anchors. The system fills in everything between them.**

```
Timeline (frames):
0         10        20        30        40        50
├─────────┼─────────┼─────────┼─────────┼─────────┤

HueShift: ●────────────────────●─────────────────●
          0°                 180°               360°
          (ease-in)          (ease-out)

Noise:            ●──────────●
                 0.0        0.8
                 (linear)

PixelSort:                         ●────────────●
                                  0.2           0.9
                                  (bounce)
```

Each parameter track has independent keyframes and easing curves.
Parameters without keyframes maintain their current static value for the
entire animation.

---

## 4. Data Structures

### 4.1 Keyframe

```rust
/// A single keyframe for one parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    /// Frame index on the timeline (0-based).
    pub frame: u32,
    /// Parameter value at this keyframe.
    pub value: f32,
    /// Easing curve applied from this keyframe to the next.
    pub easing: EasingCurve,
}
```

### 4.2 EasingCurve

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
pub enum EasingCurve {
    #[default]
    Linear,
    EaseIn,          // t²
    EaseOut,         // 1 - (1-t)²
    EaseInOut,       // smoothstep
    EaseInCubic,     // t³
    EaseOutCubic,    // 1 - (1-t)³
    EaseInOutCubic,  // cubic smoothstep
    EaseInElastic,   // spring-like overshoot at start
    EaseOutElastic,  // spring-like overshoot at end
    EaseOutBounce,   // bouncing ball effect
    Step,            // instant jump (no interpolation)
    Hold,            // maintain previous value until next keyframe
    CubicBezier {    // custom bezier (like CSS cubic-bezier)
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

impl EasingCurve {
    /// Evaluate the easing function at normalised time t ∈ [0, 1].
    /// Returns the interpolation factor ∈ [0, 1] (may exceed for elastic/bounce).
    pub fn evaluate(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 { 2.0 * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            },
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 { 4.0 * t * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            },
            Self::EaseInElastic => {
                // Standard elastic easing coefficients:
                // 10.0 controls the exponential decay rate
                // 10.75 and 0.75 offset the sine wave to start/end at zero
                // c = 2π/3 sets the oscillation period
                if t == 0.0 || t == 1.0 { return t; }
                let c = (2.0 * std::f32::consts::PI) / 3.0;
                -(2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c).sin())
            },
            Self::EaseOutElastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let c = (2.0 * std::f32::consts::PI) / 3.0;
                2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
            },
            Self::EaseOutBounce => bounce_out(t),
            Self::Step => if t < 1.0 { 0.0 } else { 1.0 },
            Self::Hold => 0.0,  // always returns start value
            Self::CubicBezier { x1, y1, x2, y2 } => {
                cubic_bezier_evaluate(x1, y1, x2, y2, t)
            },
        }
    }
}

fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}
```

### 4.3 ParameterTrack

```rust
/// A sequence of keyframes for a single effect parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterTrack {
    /// Index of the effect in the pipeline.
    pub effect_idx: usize,
    /// Index of the parameter within that effect's param_descriptors().
    pub param_idx: usize,
    /// Human-readable label (e.g., "HueShift → degrees").
    pub label: String,
    /// Ordered keyframes (sorted by frame index).
    pub keyframes: Vec<Keyframe>,
}

impl ParameterTrack {
    /// Evaluate the track at a given frame, interpolating between keyframes.
    pub fn evaluate(&self, frame: u32) -> Option<f32> {
        if self.keyframes.is_empty() { return None; }
        if self.keyframes.len() == 1 { return Some(self.keyframes[0].value); }

        // Before first keyframe: hold first value
        if frame <= self.keyframes[0].frame {
            return Some(self.keyframes[0].value);
        }

        // After last keyframe: hold last value
        let last = &self.keyframes[self.keyframes.len() - 1];
        if frame >= last.frame {
            return Some(last.value);
        }

        // Find surrounding keyframes
        for window in self.keyframes.windows(2) {
            let (kf_a, kf_b) = (&window[0], &window[1]);
            if frame >= kf_a.frame && frame < kf_b.frame {
                let t = (frame - kf_a.frame) as f32
                      / (kf_b.frame - kf_a.frame) as f32;
                let eased_t = kf_a.easing.evaluate(t);
                return Some(lerp(kf_a.value, kf_b.value, eased_t));
            }
        }

        None
    }
}
```

### 4.4 KeyframeTimeline

```rust
/// The full keyframe animation timeline.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KeyframeTimeline {
    /// Total duration in frames.
    pub total_frames: u32,
    /// Frames per second for playback.
    pub fps: u32,
    /// Parameter tracks (one per animated parameter).
    pub tracks: Vec<ParameterTrack>,
    /// Playback loop mode.
    pub loop_mode: LoopMode,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum LoopMode {
    #[default]
    Once,
    Loop,
    PingPong,
    Reverse,
}
```

### 4.5 AppState Changes

```rust
// In AppState:
pub keyframe_timeline: KeyframeTimeline,
pub keyframe_editor_open: bool,
pub keyframe_playhead: u32,  // current frame position for scrubbing
pub keyframe_selected_track: usize,
pub keyframe_selected_keyframe: usize,
```

---

## 5. TUI Layout

### 5.1 Keyframe Editor Panel

The keyframe editor replaces the standard Animation Panel when opened
via `Ctrl+K`. It occupies the bottom 12 lines of the terminal.

```
┌──────────────────────────────────────────────────────────────────┐
│ Status Bar                                                       │
├──────────────────────────────────────┬───────────────────────────┤
│                                      │                           │
│  Canvas (reduced height)             │  Effects Panel             │
│                                      │                           │
├──────────────────────────────────────┴───────────────────────────┤
│ Keyframe Editor                    ▶ Frame 15/60  2.0s  24fps    │
│                                                                  │
│ Timeline: 0    10    20    30    40    50    60                   │
│           ├─────┼─────┼─────┼─────┼─────┼─────┤                 │
│                      ▲ playhead                                  │
│                                                                  │
│ HueShift.deg   ●─────────────●──────────────────●               │
│                0°           180°                360°  (EaseIn)   │
│ Noise.int            ●──────●                                    │
│                     0.0    0.8                        (Linear)   │
│ PixelSort.thr                     ●────────────●                 │
│                                  0.2          0.9     (Bounce)   │
│                                                                  │
│ k:add keyframe  d:delete  e:easing  Space:play  ←/→:scrub       │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Track Visualisation

Each track is rendered as a horizontal line with `●` markers at keyframe
positions. The line between keyframes is drawn with characters that hint at
the easing curve:

| Easing | Line Style |
|--------|------------|
| Linear | `────` (solid dash) |
| EaseIn | `╌───` (dotted → solid, accelerating) |
| EaseOut | `───╌` (solid → dotted, decelerating) |
| EaseInOut | `╌──╌` (dotted → solid → dotted) |
| Step | `│` (vertical jump) |
| Elastic/Bounce | `~───` (tilde for overshoot hint) |

### 5.3 Playhead

The playhead is a vertical line `▲` that shows the current frame position.
During playback it moves right automatically. During scrubbing it follows
the `←` / `→` keys.

The canvas updates in real-time as the playhead moves, showing the
interpolated effect state at that frame position.

---

## 6. Keyboard Shortcuts

### 6.1 Normal Mode

| Key | Action |
|-----|--------|
| `Ctrl+K` | Toggle Keyframe Editor panel open/closed |

### 6.2 Keyframe Editor Focused

| Key | Action |
|-----|--------|
| `↑` / `k` | Select previous track |
| `↓` / `j` | Select next track |
| `←` / `h` | Move playhead left (1 frame) |
| `→` / `l` | Move playhead right (1 frame) |
| `Shift+←` / `Shift+→` | Move playhead left/right by 10 frames |
| `Home` | Jump to frame 0 |
| `End` | Jump to last frame |
| `Enter` | Add/edit keyframe at playhead position on selected track |
| `d` / `Delete` | Delete selected keyframe |
| `e` | Cycle easing curve for selected keyframe |
| `Shift+E` | Open easing picker dialog |
| `n` | New track — opens effect/parameter picker |
| `Shift+D` | Delete selected track (with confirmation) |
| `t` | Set total frame count |
| `f` | Set FPS |
| `m` | Cycle loop mode (Once → Loop → PingPong → Reverse) |
| `Space` | Play / pause animation preview |
| `r` | Render all frames (generate pipeline snapshots) |
| `Ctrl+E` | Export rendered animation |
| `Esc` | Return focus to Effects panel |
| `Tab` | Cycle focus to next panel |

### 6.3 Keyframe Value Dialog

Opened with `Enter` on a track at the playhead position:

| Key | Action |
|-----|--------|
| `←` / `→` | Decrease / increase value |
| `Shift+←` / `Shift+→` | Coarse adjustment |
| Number keys | Direct numeric input |
| `Enter` | Confirm value |
| `Esc` | Cancel |

### 6.4 Easing Picker Dialog

```
┌──────────────────────────────────────────┐
│ Easing Curve                             │
│                                          │
│ ► Linear           ╱                     │
│   Ease In         ╱                      │
│   Ease Out          ╱                    │
│   Ease In/Out       ╱                    │
│   Ease In Cubic   ╱                      │
│   Ease Out Cubic    ╱                    │
│   Elastic Out     ~╱                     │
│   Bounce Out      ╱                      │
│   Step            │                      │
│   Hold            ─                      │
│   Custom Bezier   ╱                      │
│                                          │
│ Enter: Select  │  Esc: Cancel            │
└──────────────────────────────────────────┘
```

Each option shows a mini ASCII preview of the curve shape.

### 6.5 Track Picker Dialog

Opened with `n` (new track):

```
┌──────────────────────────────────────────┐
│ Add Parameter Track                      │
│                                          │
│ Effect:                                  │
│ ► [0] HueShift                           │
│   [1] Noise                              │
│   [2] PixelSort                          │
│                                          │
│ Parameter:                               │
│ ► degrees  [0.0 — 360.0]                │
│                                          │
│ Enter: Add Track  │  Esc: Cancel         │
└──────────────────────────────────────────┘
```

---

## 7. Render Pipeline

### 7.1 Frame Generation

When the user presses `r` (render) or `Space` (play), the system generates
pipeline snapshots for every frame:

```rust
fn generate_frame_pipelines(
    base_pipeline: &Pipeline,
    timeline: &KeyframeTimeline,
) -> Vec<Pipeline> {
    (0..timeline.total_frames)
        .map(|frame| {
            let mut pipeline = base_pipeline.clone();
            for track in &timeline.tracks {
                if let Some(value) = track.evaluate(frame) {
                    pipeline.effects[track.effect_idx]
                        .effect
                        .apply_params(track.param_idx, value);
                }
            }
            pipeline
        })
        .collect()
}
```

### 7.2 Worker Integration

Generated pipelines are dispatched to the worker thread using the existing
`RenderSweepBatch` command:

```rust
WorkerCommand::RenderSweepBatch {
    image: proxy_asset.clone(),
    pipelines: generate_frame_pipelines(&pipeline, &timeline),
    response_tx: resp_tx,
}
```

### 7.3 Scrubbing (Real-Time Preview)

When the user scrubs the playhead, only the current frame's pipeline is
computed:

```rust
fn preview_at_playhead(app: &mut AppState) {
    let pipeline = generate_single_frame_pipeline(
        &app.pipeline,
        &app.keyframe_timeline,
        app.keyframe_playhead,
    );
    app.dispatch_process(pipeline);
}
```

This uses the existing single-frame `WorkerCommand::Process` for fast
response.

---

## 8. Pipeline Persistence

Keyframe timelines are saved alongside the pipeline:

```json
{
  "effects": [ ... ],
  "keyframe_timeline": {
    "total_frames": 60,
    "fps": 24,
    "loop_mode": "PingPong",
    "tracks": [
      {
        "effect_idx": 0,
        "param_idx": 0,
        "label": "HueShift → degrees",
        "keyframes": [
          { "frame": 0, "value": 0.0, "easing": "EaseIn" },
          { "frame": 30, "value": 180.0, "easing": "EaseOut" },
          { "frame": 60, "value": 360.0, "easing": "Linear" }
        ]
      },
      {
        "effect_idx": 1,
        "param_idx": 0,
        "label": "Noise → intensity",
        "keyframes": [
          { "frame": 10, "value": 0.0, "easing": "Linear" },
          { "frame": 25, "value": 0.8, "easing": "Linear" }
        ]
      }
    ]
  }
}
```

Loading a pipeline without `keyframe_timeline` creates an empty timeline
(backward compatible).

---

## 9. Migration from Existing Animation

The existing `AnimationTimeline` (frame-by-frame capture + single-parameter
sweep) coexists with the new `KeyframeTimeline`. Users choose between:

- **Frame-by-frame** (`Ctrl+N`): manual capture, each frame is independent
- **Keyframe** (`Ctrl+K`): parametric, interpolated, multi-parameter

Both can produce the same `Vec<Pipeline>` for rendering and export. The
keyframe system is strictly more powerful but the frame-by-frame approach
remains simpler for quick captures.

---

## 10. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| Track references deleted effect | Track is marked `[⚠ orphaned]`; ignored during render |
| Track references out-of-range param | Track is marked `[⚠ invalid]`; ignored during render |
| Keyframe value exceeds param min/max | Clamped to valid range silently |
| Zero total frames | Set minimum to 2 frames |
| Duplicate keyframe at same frame | Later keyframe replaces earlier one |
| Empty timeline on render/export | Show `"Add at least one track with keyframes"` |

---

## 11. File Changes Summary

| File | Change |
|------|--------|
| `src/app/keyframes.rs` *(new)* | `Keyframe`, `EasingCurve`, `ParameterTrack`, `KeyframeTimeline`, `LoopMode` structs; interpolation logic |
| `src/app/state.rs` | Add `keyframe_timeline`, `keyframe_editor_open`, `keyframe_playhead`, track/keyframe selection |
| `src/app/handlers.rs` | `Ctrl+K` toggle; keyframe editor keyboard handlers |
| `src/app/dialogs.rs` | `EasingPickerDialog`, `TrackPickerDialog`, `KeyframeValueDialog` |
| `src/app/mod.rs` | Re-export keyframes module; integrate playhead scrubbing into event loop |
| `src/ui/keyframe_editor.rs` *(new)* | Keyframe editor panel widget: timeline ruler, tracks, keyframe markers, playhead |
| `src/ui/mod.rs` | Register keyframe editor in layout |
| `src/ui/layout.rs` | Add keyframe editor row to layout (12 lines when open) |
| `src/config/parser.rs` | Serialize/deserialize `KeyframeTimeline` |
| `README.md` | Document keyframe animation, shortcuts, easing curves |

---

## 12. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Data model** | `Keyframe`, `EasingCurve`, `ParameterTrack`, `KeyframeTimeline` structs; interpolation math | Compiling model, unit tests for all easing curves |
| **2 — Easing library** | Full `EasingCurve::evaluate()` with all 12+ easing functions | Verified easing output |
| **3 — Panel UI** | Keyframe editor widget: timeline ruler, track list, playhead | Visual editor (no interaction) |
| **4 — Track management** | Add/delete tracks via picker dialog; add/delete keyframes | CRUD for tracks and keyframes |
| **5 — Keyframe editing** | Value dialog, easing picker, drag keyframes | Full keyframe authoring |
| **6 — Playhead scrubbing** | `←`/`→` moves playhead; canvas updates in real-time | Interactive scrubbing |
| **7 — Playback** | `Space` plays animation; loop modes; frame rate control | In-app animation preview |
| **8 — Batch render** | Generate all frame pipelines; dispatch to worker | Full animation rendered |
| **9 — Export** | Wire to existing animation export (GIF/WebP) | Keyframe animations exportable |
| **10 — Persistence** | Save/load keyframe timelines in pipeline JSON | Reloadable keyframe animations |

---

## 13. Open Questions

1. **Graph editor:** Should the keyframe editor support a curve graph view
   (like After Effects' Graph Editor) showing the easing curve visually?
   Recommendation: defer to Phase 11 — the ASCII mini-curve in the easing
   picker is sufficient initially.

2. **Multi-parameter keyframes:** Should a single keyframe be able to set
   values for multiple parameters simultaneously (like a "snapshot")?
   Recommendation: keep tracks independent — it's simpler and more flexible.

3. **Expression-based tracks:** Should tracks support mathematical expressions
   (e.g., `sin(frame * 0.1) * 180`) instead of discrete keyframes?
   Recommendation: defer — expressions add complexity and a mini-language.

4. **Bezier curve editing:** The `CubicBezier` easing variant needs control
   point editing. Should this be a full 2D curve editor in the TUI?
   Recommendation: start with preset control points (CSS-like presets:
   `ease`, `ease-in`, `ease-out`, `ease-in-out`) and allow numeric input
   for custom control points.

5. **Integration with frame-by-frame mode:** Should keyframe-generated frames
   be editable frame-by-frame after rendering (i.e., bake keyframes into
   discrete frames)? Recommendation: yes — add a "Bake to frames" action
   that converts the keyframe timeline into an `AnimationTimeline` with
   individually editable frames.
