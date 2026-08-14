//! Tauri shell entry: spawns the DeepSeek Harness runtime, waits for the
//! loopback URL it publishes, and opens one webview window on that URL. The
//! window renders the product's own Web surface; this crate contributes only
//! process supervision. Closing the window terminates the harness.

mod harness;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Process state shared between setup, the exit handler, and the monitor.
struct AppState {
    harness_pid: Mutex<Option<u32>>,
    shutting_down: Arc<AtomicBool>,
}

fn build_window(app: &tauri::App, url: WebviewUrl, title: &str) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "main", url)
        .title(title)
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        .center()
        .build()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shutting_down = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .manage(AppState {
            harness_pid: Mutex::new(None),
            shutting_down: shutting_down.clone(),
        })
        .setup(move |app| match harness::spawn_and_wait() {
            Ok(spawned) => {
                let pid = spawned.pid();
                let url = spawned.url.clone();
                build_window(app, WebviewUrl::External(url), "DeepSeek Harness")?;
                app.state::<AppState>()
                    .harness_pid
                    .lock()
                    .unwrap()
                    .replace(pid);
                let flag = shutting_down.clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let mut child = spawned.child;
                    let _ = child.wait();
                    if !flag.load(Ordering::SeqCst) {
                        eprintln!("[dsh-desktop] harness exited; closing the shell");
                        handle.exit(1);
                    }
                });
                Ok(())
            }
            Err(message) => {
                eprintln!("[dsh-desktop] {message}");
                build_window(
                    app,
                    WebviewUrl::App("index.html".into()),
                    "DeepSeek Harness - failed to start",
                )?;
                Ok(())
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building the Tauri shell")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                state.shutting_down.store(true, Ordering::SeqCst);
                let pid = state.harness_pid.lock().unwrap().take();
                if let Some(pid) = pid {
                    harness::terminate(pid);
                }
            }
        });
}
