//! One-shot startup presentation shared by the desktop platforms.
//!
//! Native show requests are not necessarily synchronous (notably on GTK).
//! Wait for visibility instead of retrying focus or using a startup timer.

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

pub(super) const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum StartupPresentation {
    #[default]
    WaitingForReady,
    WaitingToShow,
    WaitingForVisibility,
    Finished,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    None,
    Show,
    CheckVisibility,
}

impl StartupPresentation {
    /// Called on the event-loop thread. This never waits for page loading.
    pub(super) fn on_event(&mut self, app: &AppHandle, event: &RunEvent) {
        let action = self.action(event);
        if action == Action::None {
            return;
        }
        let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
            self.cancel();
            return;
        };

        if action == Action::Show {
            // GTK temporarily disables accept-focus for focused(false) until
            // the first draw. Restore it before mapping, not after that draw.
            #[cfg(target_os = "linux")]
            if let Err(error) = window.set_focusable(true) {
                eprintln!("[dsh-desktop] failed to enable startup window focus: {error}");
            }
            if let Err(error) = window.show() {
                self.cancel();
                eprintln!("[dsh-desktop] failed to show the startup window: {error}");
                return;
            }
        }

        let visibility = window
            .is_visible()
            .and_then(|visible| window.is_minimized().map(|minimized| (visible, minimized)));
        let (visible, minimized) = match visibility {
            Ok(state) => state,
            Err(error) => {
                self.cancel();
                eprintln!("[dsh-desktop] failed to inspect the startup window: {error}");
                return;
            }
        };
        if !self.take_focus(visible, minimized) {
            return;
        }

        // Retired before native calls, including failures: never reclaim focus
        // after later app switches, page loads, reloads, or display changes.
        // Activation remains subject to the OS/window manager's focus policy.
        if let Err(error) = window.set_focus() {
            eprintln!("[dsh-desktop] failed to activate the startup window: {error}");
        }
        // WebviewWindow::set_focus only focuses the native window. Also focus
        // WKWebView / WebView2 / WebKitGTK through Tauri's portable Webview API.
        if let Err(error) = window.as_ref().set_focus() {
            eprintln!("[dsh-desktop] failed to focus the startup webview: {error}");
        }
    }

    fn action(&mut self, event: &RunEvent) -> Action {
        match event {
            RunEvent::Ready if *self == Self::WaitingForReady => {
                *self = Self::WaitingToShow;
            }
            // On macOS setup and Ready still run inside AppKit's launch
            // callback. Other platforms also get an explicit post-setup gate.
            RunEvent::MainEventsCleared => match self {
                Self::WaitingToShow => {
                    *self = Self::WaitingForVisibility;
                    return Action::Show;
                }
                Self::WaitingForVisibility => return Action::CheckVisibility,
                _ => {}
            },
            RunEvent::WindowEvent { label, event, .. } => self.on_window_event(label, event),
            RunEvent::Exit => self.cancel(),
            _ => {}
        }
        Action::None
    }

    fn on_window_event(&mut self, label: &str, event: &WindowEvent) {
        if label != MAIN_WINDOW_LABEL {
            return;
        }
        match event {
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => self.cancel(),
            WindowEvent::Focused(false) if *self == Self::WaitingForVisibility => self.cancel(),
            _ => {}
        }
    }

    fn take_focus(&mut self, visible: bool, minimized: bool) -> bool {
        if *self != Self::WaitingForVisibility {
            return false;
        }
        if minimized {
            self.cancel();
            return false;
        }
        if visible {
            self.cancel();
            return true;
        }
        false
    }

    fn cancel(&mut self) {
        *self = Self::Finished;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown() -> StartupPresentation {
        let mut startup = StartupPresentation::default();
        assert_eq!(startup.action(&RunEvent::Ready), Action::None);
        assert_eq!(startup.action(&RunEvent::MainEventsCleared), Action::Show);
        startup
    }

    fn assert_retired(startup: &mut StartupPresentation) {
        for event in [
            RunEvent::Ready,
            RunEvent::Resumed,
            RunEvent::MainEventsCleared,
        ] {
            assert_eq!(startup.action(&event), Action::None);
        }
        assert!(!startup.take_focus(true, false));
        assert_eq!(*startup, StartupPresentation::Finished);
    }

    #[test]
    fn waits_for_ready_and_a_subsequent_event_loop_drain() {
        let mut startup = StartupPresentation::default();
        for event in [RunEvent::MainEventsCleared, RunEvent::Resumed] {
            assert_eq!(startup.action(&event), Action::None);
            assert_eq!(startup, StartupPresentation::WaitingForReady);
        }
        assert_eq!(startup.action(&RunEvent::Ready), Action::None);
        assert!(!startup.take_focus(true, false));
        assert_eq!(startup.action(&RunEvent::Resumed), Action::None);
        assert_eq!(startup.action(&RunEvent::MainEventsCleared), Action::Show);
    }

    #[test]
    fn synchronous_show_can_focus_once_in_the_same_drain() {
        let mut startup = shown();
        assert!(startup.take_focus(true, false));
        assert_retired(&mut startup);
    }

    #[test]
    fn asynchronous_show_waits_for_visibility_without_repeating_show() {
        let mut startup = shown();
        for _ in 0..3 {
            assert!(!startup.take_focus(false, false));
            assert_eq!(startup.action(&RunEvent::Ready), Action::None);
            assert_eq!(
                startup.action(&RunEvent::MainEventsCleared),
                Action::CheckVisibility
            );
        }
        assert!(startup.take_focus(true, false));
        assert_retired(&mut startup);
    }

    #[test]
    fn minimizing_before_focus_retires_startup_even_if_still_hidden() {
        for visible in [false, true] {
            let mut startup = shown();
            assert!(!startup.take_focus(visible, true));
            assert_retired(&mut startup);
        }
    }

    #[test]
    fn cancelled_or_failed_presentation_never_retries() {
        let mut startup = shown();
        startup.cancel(); // Missing window, show failure, or visibility query failure.
        assert_retired(&mut startup);
    }

    #[test]
    fn main_window_blur_or_destruction_cancels_pending_focus() {
        for event in [WindowEvent::Focused(false), WindowEvent::Destroyed] {
            let mut startup = shown();
            startup.on_window_event(MAIN_WINDOW_LABEL, &event);
            assert_retired(&mut startup);
        }
    }

    #[test]
    fn destruction_before_show_prevents_presentation() {
        for ready in [false, true] {
            let mut startup = StartupPresentation::default();
            if ready {
                assert_eq!(startup.action(&RunEvent::Ready), Action::None);
            }
            startup.on_window_event(MAIN_WINDOW_LABEL, &WindowEvent::Destroyed);
            assert_retired(&mut startup);
        }
    }

    #[test]
    fn ignores_initial_blur_other_windows_and_focus_gain() {
        let mut startup = StartupPresentation::default();
        startup.on_window_event(MAIN_WINDOW_LABEL, &WindowEvent::Focused(false));
        assert_eq!(startup.action(&RunEvent::Ready), Action::None);
        startup.on_window_event(MAIN_WINDOW_LABEL, &WindowEvent::Focused(false));
        assert_eq!(startup.action(&RunEvent::MainEventsCleared), Action::Show);
        startup.on_window_event("popup", &WindowEvent::Focused(false));
        startup.on_window_event("popup", &WindowEvent::Destroyed);
        startup.on_window_event(MAIN_WINDOW_LABEL, &WindowEvent::Focused(true));
        assert!(startup.take_focus(true, false));
    }

    #[test]
    fn exit_before_ready_prevents_presentation() {
        let mut startup = StartupPresentation::default();
        assert_eq!(startup.action(&RunEvent::Exit), Action::None);
        assert_retired(&mut startup);
    }
}
