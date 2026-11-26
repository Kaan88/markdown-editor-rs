//! Restore a modern saved position only on its original monitor.

use mdshared_ui::window_state::{self, Monitor, Placement, WindowState};

pub(super) fn restore(state: &WindowState, monitors: &[Monitor]) -> Option<Placement> {
    // Do not take the toolkit's nearest-monitor fallback: on a missing display the OS
    // places the new window inside its work area, rather than at a synthesized position.
    monitors
        .iter()
        .any(|m| {
            m.name.as_deref().unwrap_or("") == state.monitor.name.as_deref().unwrap_or("")
                && m.position == state.monitor.position
                && m.size == state.monitor.size
        })
        .then_some(())?;
    window_state::restore(state, monitors)
}

/// The pre-creation monitor handle exposes the screen (not all platforms expose its work
/// area until a window exists). This is only queried for a monitor-aware saved session.
pub(super) fn monitor(handle: winit::monitor::MonitorHandle) -> Monitor {
    let (p, s) = (handle.position(), handle.size());
    let scale = handle.scale_factor() as f32;
    Monitor {
        name: handle.name(),
        position: (p.x, p.y),
        size: (s.width, s.height),
        scale,
        work: mdshared_ui::gfx::Rect::new(
            0.0,
            0.0,
            s.width as f32 / scale,
            s.height as f32 / scale,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_ui::gfx::Rect;

    #[test]
    fn a_saved_monitor_is_restored_but_no_position_is_chosen_for_a_missing_one() {
        let secondary = Monitor {
            name: Some("left".into()),
            position: (-1600, 0),
            size: (1600, 1000),
            scale: 1.25,
            work: Rect::new(0.0, 30.0, 1280.0, 770.0),
        };
        let primary = Monitor {
            name: Some("main".into()),
            position: (0, 0),
            ..secondary.clone()
        };
        let state = WindowState {
            rect: Rect::new(50.0, 4.0, 720.0, 600.0),
            monitor: secondary.id(),
            maximized: false,
            scale: 1.0,
        };
        let session = crate::session::Session {
            window_state: Some(state.to_setting()),
            ..Default::default()
        };
        let saved = crate::session::Session::parse(&session.serialize());
        let state = WindowState::from_setting(saved.window_state.as_deref().unwrap()).unwrap();
        let p = restore(&state, &[primary.clone(), secondary.clone()]).unwrap();
        assert_eq!(p.monitor, 1);
        assert_eq!(
            (p.rect.x, p.rect.y, p.rect.w, p.rect.h),
            (-1537, 38, 900, 750)
        );
        assert!(restore(&state, std::slice::from_ref(&primary)).is_none());
        let replacement = Monitor {
            name: Some("replacement".into()),
            ..secondary
        };
        assert!(restore(&state, &[primary, replacement]).is_none());
        assert!(restore(&state, &[]).is_none());
    }
}
