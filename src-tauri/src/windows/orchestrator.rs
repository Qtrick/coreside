//! Protected main-window expansion orchestrator.
//!
//! Bound math is pure and unit-tested. Animation runs off the main thread with
//! a single active generation so newer requests supersede older ones.

use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewWindow};
use tokio_util::sync::CancellationToken;

use crate::commands::CommandError;

pub const MAIN_LABEL: &str = "main";
pub const MIN_WIDTH: f64 = 900.0;
pub const MIN_HEIGHT: f64 = 600.0;
pub const WORK_AREA_MARGIN: f64 = 8.0;
pub const ANIMATION_MS: u64 = 200;
const TOOL_WINDOW_MIN_W: f64 = 400.0;
const TOOL_WINDOW_MIN_H: f64 = 320.0;
const TOOL_WINDOW_DEFAULT_W: f64 = 720.0;
const TOOL_WINDOW_DEFAULT_H: f64 = 640.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowBounds {
    pub fn right(self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(self) -> f64 {
        self.y + self.height
    }

    pub fn inset(self, margin: f64) -> Self {
        Self {
            x: self.x + margin,
            y: self.y + margin,
            width: (self.width - margin * 2.0).max(0.0),
            height: (self.height - margin * 2.0).max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpandDirection {
    Right,
    Left,
    Down,
    Up,
    Balanced,
    Automatic,
}

impl ExpandDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Left => "left",
            Self::Down => "down",
            Self::Up => "up",
            Self::Balanced => "balanced",
            Self::Automatic => "automatic",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ExpansionDecision {
    #[serde(rename_all = "camelCase")]
    Expand {
        reason: String,
        tool_id: String,
        from: WindowBounds,
        to: WindowBounds,
        direction: String,
        restore_eligible: bool,
    },
    #[serde(rename_all = "camelCase")]
    NoOp {
        reason: String,
        tool_id: Option<String>,
        from: WindowBounds,
        restore_eligible: bool,
    },
    #[serde(rename_all = "camelCase")]
    CannotSatisfy {
        reason: String,
        tool_id: Option<String>,
        from: WindowBounds,
        best_effort: Option<WindowBounds>,
        restore_eligible: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorInspect {
    pub bounds: WindowBounds,
    pub maximized: bool,
    pub fullscreen: bool,
    pub minimized: bool,
    pub scale_factor: f64,
    pub animating: bool,
    pub restore_eligible: bool,
    /// Usable work area (already margin-aware summary for the UI — not a raw monitor dump).
    pub work_area: WindowBounds,
    pub can_expand: bool,
}

#[derive(Debug, Clone)]
pub struct ExpansionPlan {
    pub to: WindowBounds,
    pub direction: ExpandDirection,
    pub fully_satisfied: bool,
}

struct RestorationSnapshot {
    bounds: WindowBounds,
    tool_id: String,
}

struct OrchestratorRuntime {
    generation: u64,
    cancel: Option<CancellationToken>,
    snapshot: Option<RestorationSnapshot>,
    animating: bool,
}

impl Default for OrchestratorRuntime {
    fn default() -> Self {
        Self {
            generation: 0,
            cancel: None,
            snapshot: None,
            animating: false,
        }
    }
}

// ponytail: process-local animation/restore state; fine for single desktop app.
static RUNTIME: Lazy<Mutex<OrchestratorRuntime>> =
    Lazy::new(|| Mutex::new(OrchestratorRuntime::default()));

fn finite(v: f64, label: &str) -> Result<f64, CommandError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(CommandError::new(
            "invalid",
            format!("{label} must be a finite number"),
        ))
    }
}

fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_bounds(from: WindowBounds, to: WindowBounds, t: f64) -> WindowBounds {
    WindowBounds {
        x: lerp(from.x, to.x, t),
        y: lerp(from.y, to.y, t),
        width: lerp(from.width, to.width, t),
        height: lerp(from.height, to.height, t),
    }
}

/// Place a target size inside `usable` according to preferred direction.
pub fn place_expanded_bounds(
    current: WindowBounds,
    usable: WindowBounds,
    target_w: f64,
    target_h: f64,
    direction: ExpandDirection,
) -> WindowBounds {
    let w = target_w.clamp(MIN_WIDTH.min(usable.width), usable.width);
    let h = target_h.clamp(MIN_HEIGHT.min(usable.height), usable.height);
    let dir = match direction {
        ExpandDirection::Automatic => pick_automatic(current, usable),
        other => other,
    };

    let (mut x, mut y) = match dir {
        ExpandDirection::Right => (current.x, current.y),
        ExpandDirection::Left => (current.right() - w, current.y),
        ExpandDirection::Down => (current.x, current.y),
        ExpandDirection::Up => (current.x, current.bottom() - h),
        ExpandDirection::Balanced => (
            current.x + (current.width - w) / 2.0,
            current.y + (current.height - h) / 2.0,
        ),
        ExpandDirection::Automatic => unreachable!(),
    };

    if x + w > usable.right() {
        x = usable.right() - w;
    }
    if x < usable.x {
        x = usable.x;
    }
    if y + h > usable.bottom() {
        y = usable.bottom() - h;
    }
    if y < usable.y {
        y = usable.y;
    }

    WindowBounds {
        x,
        y,
        width: w.min(usable.width),
        height: h.min(usable.height),
    }
}

fn pick_automatic(current: WindowBounds, usable: WindowBounds) -> ExpandDirection {
    let right = (usable.right() - current.right()).max(0.0);
    let left = (current.x - usable.x).max(0.0);
    let down = (usable.bottom() - current.bottom()).max(0.0);
    let up = (current.y - usable.y).max(0.0);
    if right.max(left) >= down.max(up) {
        if right >= left {
            ExpandDirection::Right
        } else {
            ExpandDirection::Left
        }
    } else if down >= up {
        ExpandDirection::Down
    } else {
        ExpandDirection::Up
    }
}

/// Pure expansion planner. `add_w`/`add_h` are required additional logical pixels.
pub fn compute_expansion(
    current: WindowBounds,
    work_area: WindowBounds,
    add_w: f64,
    add_h: f64,
    direction: ExpandDirection,
) -> Result<ExpansionPlan, &'static str> {
    let add_w = add_w.max(0.0);
    let add_h = add_h.max(0.0);
    if add_w == 0.0 && add_h == 0.0 {
        return Err("already_satisfies_minimum");
    }

    let usable = work_area.inset(WORK_AREA_MARGIN);
    if usable.width < MIN_WIDTH || usable.height < MIN_HEIGHT {
        return Err("work_area_too_small");
    }

    let target_w = (current.width + add_w).clamp(MIN_WIDTH, usable.width);
    let target_h = (current.height + add_h).clamp(MIN_HEIGHT, usable.height);
    let resolved = match direction {
        ExpandDirection::Automatic => pick_automatic(current, usable),
        other => other,
    };
    let to = place_expanded_bounds(current, usable, target_w, target_h, resolved);

    if (to.width - current.width).abs() < 0.5 && (to.height - current.height).abs() < 0.5 {
        return Err("no_room_to_expand");
    }

    let want_w = (current.width + add_w).min(usable.width);
    let want_h = (current.height + add_h).min(usable.height);
    let fully_satisfied = to.width + 0.5 >= want_w && to.height + 0.5 >= want_h;

    Ok(ExpansionPlan {
        to,
        direction: resolved,
        fully_satisfied,
    })
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, CommandError> {
    app.get_webview_window(MAIN_LABEL)
        .ok_or_else(|| CommandError::new("window", "Main window is not available."))
}

fn read_logical_outer(window: &WebviewWindow) -> Result<(WindowBounds, f64), CommandError> {
    let scale = window
        .scale_factor()
        .map_err(|e| CommandError::new("window", format!("scale_factor failed: {e}")))?;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let pos = window
        .outer_position()
        .map_err(|e| CommandError::new("window", format!("outer_position failed: {e}")))?;
    let size = window
        .outer_size()
        .map_err(|e| CommandError::new("window", format!("outer_size failed: {e}")))?;
    let logical_pos = pos.to_logical::<f64>(scale);
    let logical_size = size.to_logical::<f64>(scale);
    Ok((
        WindowBounds {
            x: logical_pos.x,
            y: logical_pos.y,
            width: logical_size.width,
            height: logical_size.height,
        },
        scale,
    ))
}

fn read_work_area_logical(
    window: &WebviewWindow,
    scale: f64,
) -> Result<WindowBounds, CommandError> {
    let monitor = window
        .current_monitor()
        .map_err(|e| CommandError::new("window", format!("current_monitor failed: {e}")))?
        .ok_or_else(|| CommandError::new("window", "No current monitor."))?;
    let area = monitor.work_area();
    let pos = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);
    Ok(WindowBounds {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn chrome_delta_logical(window: &WebviewWindow, scale: f64) -> Result<(f64, f64), CommandError> {
    let outer = window
        .outer_size()
        .map_err(|e| CommandError::new("window", format!("outer_size failed: {e}")))?;
    let inner = window
        .inner_size()
        .map_err(|e| CommandError::new("window", format!("inner_size failed: {e}")))?;
    let dw = (outer.width as f64 - inner.width as f64) / scale;
    let dh = (outer.height as f64 - inner.height as f64) / scale;
    Ok((dw.max(0.0), dh.max(0.0)))
}

fn apply_outer_bounds(
    window: &WebviewWindow,
    bounds: WindowBounds,
    chrome: (f64, f64),
) -> Result<(), CommandError> {
    let inner_w = (bounds.width - chrome.0).max(MIN_WIDTH);
    let inner_h = (bounds.height - chrome.1).max(MIN_HEIGHT);
    window
        .set_position(Position::Logical(LogicalPosition::new(bounds.x, bounds.y)))
        .map_err(|e| CommandError::new("window", format!("set_position failed: {e}")))?;
    window
        .set_size(Size::Logical(LogicalSize::new(inner_w, inner_h)))
        .map_err(|e| CommandError::new("window", format!("set_size failed: {e}")))?;
    Ok(())
}

fn begin_animation(app: AppHandle, from: WindowBounds, to: WindowBounds, reduced_motion: bool) {
    let cancel = CancellationToken::new();
    let generation = {
        let mut rt = RUNTIME.lock();
        if let Some(prev) = rt.cancel.take() {
            prev.cancel();
        }
        rt.generation = rt.generation.wrapping_add(1);
        rt.cancel = Some(cancel.clone());
        rt.animating = true;
        rt.generation
    };

    tauri::async_runtime::spawn(async move {
        let chrome = {
            let Ok(window) = main_window(&app) else {
                RUNTIME.lock().animating = false;
                return;
            };
            chrome_delta_logical(&window, window.scale_factor().unwrap_or(1.0))
                .unwrap_or((0.0, 0.0))
        };

        if reduced_motion {
            if let Ok(window) = main_window(&app) {
                let _ = apply_outer_bounds(&window, to, chrome);
            }
            let mut rt = RUNTIME.lock();
            if rt.generation == generation {
                rt.animating = false;
                rt.cancel = None;
            }
            return;
        }

        let start = Instant::now();
        let duration = Duration::from_millis(ANIMATION_MS);
        loop {
            if cancel.is_cancelled() {
                break;
            }
            {
                let rt = RUNTIME.lock();
                if rt.generation != generation {
                    break;
                }
            }

            let elapsed = start.elapsed();
            let t = (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);
            let eased = ease_out_cubic(t);
            let frame = lerp_bounds(from, to, eased);
            if let Ok(window) = main_window(&app) {
                let _ = apply_outer_bounds(&window, frame, chrome);
            }
            if t >= 1.0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(16)).await;
        }

        let mut rt = RUNTIME.lock();
        if rt.generation == generation {
            rt.animating = false;
            rt.cancel = None;
        }
    });
}

pub fn inspect(app: &AppHandle) -> Result<OrchestratorInspect, CommandError> {
    let window = main_window(app)?;
    let (bounds, scale) = read_logical_outer(&window)?;
    let work_area = read_work_area_logical(&window, scale)?;
    let maximized = window.is_maximized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let rt = RUNTIME.lock();
    Ok(OrchestratorInspect {
        bounds,
        maximized,
        fullscreen,
        minimized,
        scale_factor: scale,
        animating: rt.animating,
        restore_eligible: rt.snapshot.is_some() && !maximized && !fullscreen && !minimized,
        work_area,
        can_expand: !maximized && !fullscreen && !minimized,
    })
}

pub fn expand(
    app: &AppHandle,
    tool_id: String,
    min_useful_width: f64,
    min_useful_height: f64,
    direction: ExpandDirection,
    reduced_motion: bool,
) -> Result<ExpansionDecision, CommandError> {
    let tool_id = tool_id.trim().to_string();
    if tool_id.is_empty() {
        return Err(CommandError::new("invalid", "toolId is required"));
    }
    let min_useful_width = finite(min_useful_width, "minUsefulWidth")?;
    let min_useful_height = finite(min_useful_height, "minUsefulHeight")?;

    let window = main_window(app)?;
    let maximized = window.is_maximized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let (from, scale) = read_logical_outer(&window)?;
    let work_area = read_work_area_logical(&window, scale)?;
    let restore_eligible = RUNTIME.lock().snapshot.is_some();

    if maximized || fullscreen || minimized {
        return Ok(ExpansionDecision::NoOp {
            reason: if maximized {
                "window_maximized".into()
            } else if fullscreen {
                "window_fullscreen".into()
            } else {
                "window_minimized".into()
            },
            tool_id: Some(tool_id),
            from,
            restore_eligible,
        });
    }

    let usable = work_area.inset(WORK_AREA_MARGIN);
    let want_w = min_useful_width.clamp(MIN_WIDTH, usable.width.max(MIN_WIDTH));
    let want_h = min_useful_height.clamp(MIN_HEIGHT, usable.height.max(MIN_HEIGHT));
    let add_w = (want_w - from.width).max(0.0);
    let add_h = (want_h - from.height).max(0.0);

    match compute_expansion(from, work_area, add_w, add_h, direction) {
        Err("already_satisfies_minimum") => Ok(ExpansionDecision::NoOp {
            reason: "already_satisfies_minimum".into(),
            tool_id: Some(tool_id),
            from,
            restore_eligible,
        }),
        Err(reason) => Ok(ExpansionDecision::CannotSatisfy {
            reason: reason.into(),
            tool_id: Some(tool_id),
            from,
            best_effort: None,
            restore_eligible,
        }),
        Ok(plan) => {
            {
                let mut rt = RUNTIME.lock();
                if rt.snapshot.is_none() {
                    rt.snapshot = Some(RestorationSnapshot {
                        bounds: from,
                        tool_id: tool_id.clone(),
                    });
                }
            }
            begin_animation(app.clone(), from, plan.to, reduced_motion);
            let reason = if plan.fully_satisfied {
                "tool_below_minimum_useful_size"
            } else {
                "partial_expansion"
            };
            Ok(ExpansionDecision::Expand {
                reason: reason.into(),
                tool_id,
                from,
                to: plan.to,
                direction: plan.direction.as_str().into(),
                restore_eligible: true,
            })
        }
    }
}

pub fn restore(app: &AppHandle, reduced_motion: bool) -> Result<ExpansionDecision, CommandError> {
    let window = main_window(app)?;
    let maximized = window.is_maximized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let (from, scale) = read_logical_outer(&window)?;
    let snapshot = {
        RUNTIME
            .lock()
            .snapshot
            .as_ref()
            .map(|s| (s.bounds, s.tool_id.clone()))
    };

    let Some((to, tool_id)) = snapshot else {
        return Ok(ExpansionDecision::NoOp {
            reason: "no_snapshot".into(),
            tool_id: None,
            from,
            restore_eligible: false,
        });
    };

    if maximized || fullscreen || minimized {
        return Ok(ExpansionDecision::NoOp {
            reason: "window_not_restorable".into(),
            tool_id: Some(tool_id),
            from,
            restore_eligible: false,
        });
    }

    // Keep restore within current work area.
    let work_area = read_work_area_logical(&window, scale)?;
    let usable = work_area.inset(WORK_AREA_MARGIN);
    let clamped = place_expanded_bounds(
        to,
        usable,
        to.width.clamp(MIN_WIDTH, usable.width),
        to.height.clamp(MIN_HEIGHT, usable.height),
        ExpandDirection::Balanced,
    );

    RUNTIME.lock().snapshot = None;
    begin_animation(app.clone(), from, clamped, reduced_motion);
    Ok(ExpansionDecision::Expand {
        reason: "restore_snapshot".into(),
        tool_id,
        from,
        to: clamped,
        direction: "balanced".into(),
        restore_eligible: false,
    })
}

pub fn cancel_animation() -> Result<(), CommandError> {
    let mut rt = RUNTIME.lock();
    if let Some(prev) = rt.cancel.take() {
        prev.cancel();
    }
    rt.generation = rt.generation.wrapping_add(1);
    rt.animating = false;
    Ok(())
}

/// Clamp optional tool-window inner size to the primary monitor work area.
pub fn clamp_tool_window_size(
    app: &AppHandle,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<(f64, f64), CommandError> {
    let width = match width {
        Some(v) => finite(v, "width")?,
        None => TOOL_WINDOW_DEFAULT_W,
    };
    let height = match height {
        Some(v) => finite(v, "height")?,
        None => TOOL_WINDOW_DEFAULT_H,
    };

    let (max_w, max_h) = match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let scale = {
                let s = monitor.scale_factor();
                if s.is_finite() && s > 0.0 {
                    s
                } else {
                    1.0
                }
            };
            let area = monitor.work_area();
            let size = area.size.to_logical::<f64>(scale);
            (
                (size.width - WORK_AREA_MARGIN * 2.0).max(TOOL_WINDOW_MIN_W),
                (size.height - WORK_AREA_MARGIN * 2.0).max(TOOL_WINDOW_MIN_H),
            )
        }
        _ => (1440.0, 900.0),
    };

    Ok((
        width.clamp(TOOL_WINDOW_MIN_W, max_w),
        height.clamp(TOOL_WINDOW_MIN_H, max_h),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: f64, h: f64) -> WindowBounds {
        WindowBounds {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        }
    }

    fn at(x: f64, y: f64, w: f64, h: f64) -> WindowBounds {
        WindowBounds {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn right_expansion_keeps_left_edge_when_space_allows() {
        let current = at(100.0, 80.0, 1000.0, 700.0);
        let plan = compute_expansion(
            current,
            area(1920.0, 1080.0),
            200.0,
            0.0,
            ExpandDirection::Right,
        )
        .unwrap();
        assert_eq!(plan.direction, ExpandDirection::Right);
        assert!((plan.to.x - 100.0).abs() < 0.01);
        assert!((plan.to.width - 1200.0).abs() < 0.01);
        assert!(plan.fully_satisfied);
    }

    #[test]
    fn right_expansion_shifts_left_near_monitor_edge() {
        let work = area(1440.0, 900.0);
        let usable = work.inset(WORK_AREA_MARGIN);
        let current = at(usable.right() - 1000.0, 40.0, 1000.0, 700.0);
        let plan = compute_expansion(current, work, 300.0, 0.0, ExpandDirection::Right).unwrap();
        assert!(plan.to.right() <= usable.right() + 0.01);
        assert!(plan.to.x >= usable.x - 0.01);
        assert!(plan.to.width > current.width);
    }

    #[test]
    fn balanced_grows_around_center() {
        let current = at(200.0, 100.0, 1000.0, 700.0);
        let plan = compute_expansion(
            current,
            area(1920.0, 1080.0),
            200.0,
            100.0,
            ExpandDirection::Balanced,
        )
        .unwrap();
        let cx = current.x + current.width / 2.0;
        let cy = current.y + current.height / 2.0;
        let nx = plan.to.x + plan.to.width / 2.0;
        let ny = plan.to.y + plan.to.height / 2.0;
        assert!((cx - nx).abs() < 1.0);
        assert!((cy - ny).abs() < 1.0);
    }

    #[test]
    fn no_room_returns_error() {
        let work = area(920.0, 620.0);
        let usable = work.inset(WORK_AREA_MARGIN);
        let current = at(usable.x, usable.y, usable.width, usable.height);
        let err =
            compute_expansion(current, work, 100.0, 100.0, ExpandDirection::Right).unwrap_err();
        assert_eq!(err, "no_room_to_expand");
    }

    #[test]
    fn already_large_enough_is_noop_reason() {
        let err = compute_expansion(
            at(0.0, 0.0, 1200.0, 800.0),
            area(1920.0, 1080.0),
            0.0,
            0.0,
            ExpandDirection::Right,
        )
        .unwrap_err();
        assert_eq!(err, "already_satisfies_minimum");
    }

    #[test]
    fn clamps_to_protected_minimum() {
        let current = at(50.0, 50.0, 800.0, 500.0);
        let plan = compute_expansion(
            current,
            area(1920.0, 1080.0),
            50.0,
            50.0,
            ExpandDirection::Right,
        )
        .unwrap();
        assert!(plan.to.width >= MIN_WIDTH - 0.01);
        assert!(plan.to.height >= MIN_HEIGHT - 0.01);
    }

    #[test]
    fn automatic_prefers_side_with_more_space() {
        let current = at(20.0, 40.0, 1000.0, 700.0);
        let plan = compute_expansion(
            current,
            area(1920.0, 1080.0),
            200.0,
            0.0,
            ExpandDirection::Automatic,
        )
        .unwrap();
        assert_eq!(plan.direction, ExpandDirection::Right);
    }
}
