//! Tauri shell for the DeepSeek Harness Web surface — attach-only.
//!
//! The desktop profile's `desktop-launch` row spawns this binary as
//! `dsh-desktop --attach http://127.0.0.1:<port>` once the Web runtime
//! binds. The shell opens one webview window on the loopback URL and
//! supervises nothing — the runtime is the parent, never a child. Lifetime
//! coupling runs both ways:
//!
//! - the user closes the window: the shell exits and the plugin requests a
//!   graceful profile exit;
//! - the runtime dies first: the shell exits with it, through stdin EOF
//!   (the runtime holds the pipe) and, on unix, a parent-reparenting poll
//!   (macOS rewires the app's stdio during bootstrap, so the poll is the
//!   load-bearing link there).
//!
//! The window renders the product's own Web surface; this crate contributes
//! only process lifetime coupling, never UI.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

fn build_window(app: &tauri::App, url: WebviewUrl, title: &str) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "main", url)
        .title(title)
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        .center()
        .build()?;
    Ok(())
}

/// Parses one whitespace token as a loopback http URL with a port,
/// stripping trailing punctuation, e.g. http://127.0.0.1:3080.
pub fn parse_loopback_url(token: &str) -> Option<Url> {
    let rest = token.strip_prefix("http://")?;
    let candidate = format!(
        "http://{}",
        rest.trim_end_matches(|c: char| {
            !c.is_ascii_alphanumeric() && c != '.' && c != ':' && c != '-' && c != '/'
        })
    );
    let url = Url::parse(&candidate).ok()?;
    let host = url.host_str()?;
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
    (loopback && url.port().is_some()).then_some(url)
}

/// Parses `--attach <url>` from process arguments: `Ok(Some(url))` for a
/// valid loopback URL, `Ok(None)` when the flag is absent, and `Err` for a
/// malformed, missing, or non-loopback URL.
pub fn parse_attach_args(args: &[String]) -> Result<Option<Url>, String> {
    let mut attach = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--attach" {
            let value = iter
                .next()
                .ok_or_else(|| String::from("--attach needs a loopback URL"))?;
            let url = parse_loopback_url(value).ok_or_else(|| {
                format!("--attach URL must be a loopback http URL with a port, got {value:?}")
            })?;
            attach = Some(url);
        }
    }
    Ok(attach)
}

/// Watches for the runtime's death and exits the app with it. Two links,
/// because macOS rewires the app's stdio during bootstrap (fd 0 becomes a
/// unix socket), so the stdin pipe alone is not reliable there:
///
/// - stdin EOF: the runtime holds the other end of the stdin pipe, so EOF
///   means it died (gracefully, by signal, or by crash). This is the
///   load-bearing link where the platform keeps the pipe as fd 0 (Windows).
/// - reparenting poll (unix): when the parent process dies, the app is
///   reparented to pid 1, which the poll detects within one interval.
fn watch_runtime_exit(handle: &tauri::AppHandle) {
    {
        let handle = handle.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buffer = [0u8; 1024];
            loop {
                match std::io::stdin().lock().read(&mut buffer) {
                    Ok(0) | Err(_) => {
                        handle.exit(0);
                        return;
                    }
                    Ok(_) => {}
                }
            }
        });
    }
    #[cfg(unix)]
    {
        let handle = handle.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if unsafe { libc::getppid() } == 1 {
                handle.exit(0);
                return;
            }
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let attach = parse_attach_args(&args);

    tauri::Builder::default()
        .setup(move |app| match attach {
            Ok(Some(url)) => {
                // The runtime is our parent and already serves the Web
                // surface: open the window and watch its death.
                build_window(app, WebviewUrl::External(url), "DeepSeek Harness")?;
                let handle = app.handle().clone();
                watch_runtime_exit(&handle);
                Ok(())
            }
            Ok(None) => {
                let message = "launch through the desktop profile instead: dsh --profile desktop (no --attach given)";
                eprintln!("[dsh-desktop] {message}");
                build_window(
                    app,
                    WebviewUrl::App("index.html".into()),
                    "DeepSeek Harness - failed to start",
                )?;
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
        .run(|_app_handle, _event| {});
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attach_flag() {
        let args = vec![String::from("--attach"), String::from("http://127.0.0.1:3080")];
        let url = parse_attach_args(&args).expect("attach ok").expect("url");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(3080));
    }

    #[test]
    fn attach_absent_is_none() {
        assert!(parse_attach_args(&[]).expect("ok").is_none());
        let other = vec![String::from("--other")];
        assert!(parse_attach_args(&other).expect("ok").is_none());
    }

    #[test]
    fn attach_rejects_non_loopback_and_malformed() {
        for bad in [
            vec![String::from("--attach"), String::from("http://192.168.1.5:3080")],
            vec![String::from("--attach"), String::from("http://127.0.0.1")],
            vec![String::from("--attach"), String::from("not a url")],
            vec![String::from("--attach")],
        ] {
            assert!(parse_attach_args(&bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn scans_loopback_url_tokens() {
        let line = "dsh web: http://127.0.0.1:3080 (LAN: http://192.168.1.5:3080)";
        let found = line.split_whitespace().find_map(parse_loopback_url);
        let url = found.expect("loopback URL");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(3080));
        assert!(parse_loopback_url("http://192.168.1.5:3080").is_none());
        assert!(parse_loopback_url("http://127.0.0.1").is_none());
        assert!(parse_loopback_url("nothing here").is_none());
    }
}
