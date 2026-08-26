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
//! process lifetime coupling, follows the desktop environment's light/dark
//! window theme, and hands external links to the system browser, but never
//! renders UI content itself.

use tauri::{webview::NewWindowResponse, WebviewUrl, WebviewWindowBuilder};
use url::Url;

/// What the shell should do with a URL requested by web content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkAction {
    /// Same-origin URL of the attached Web runtime: keep it in the shell.
    KeepInWebview,
    /// Hand off to the operating system's default browser/app.
    OpenExternally,
    /// Unsupported or untrusted scheme: neither navigate nor launch an app.
    Block,
}

/// Decides how a requested link should be handled.
fn link_action(url: &Url, app_url: Option<&Url>) -> LinkAction {
    if app_url.is_some_and(|app_url| url.origin() == app_url.origin()) {
        LinkAction::KeepInWebview
    } else if matches!(url.scheme(), "http" | "https" | "mailto" | "tel") {
        LinkAction::OpenExternally
    } else {
        LinkAction::Block
    }
}

fn open_externally(url: &Url) {
    if let Err(error) = open::that_detached(url.as_str()) {
        eprintln!("[dsh-desktop] failed to open {url} with the default application: {error}");
    }
}

/// WebView-side fallback for Linux Tauri WebViews that do not expose
/// clipboard images as file items in the native `paste` event.
///
/// Instead of simulating a drag-and-drop, this reads the image through
/// Tauri's clipboard plugin and replays a normal `paste` event containing the
/// image, so the Web app's existing paste handler can add it through its own
/// image-intake path.
const PASTE_IMAGE_SCRIPT: &str = r#"
(() => {
  const KEY = '__dshDesktopClipboardImageFallback';
  if (window[KEY]) return;
  window[KEY] = true;

  const hasNativeImageFile = (event) => {
    const items = event.clipboardData && event.clipboardData.items;
    if (!items) return false;
    for (const item of items) {
      if (item.kind === 'file' && item.type && item.type.startsWith('image/')) {
        const file = item.getAsFile ? item.getAsFile() : null;
        if (file !== null) return true;
      }
    }
    return false;
  };

  const replayPasteWithImage = (target, file) => {
    const clipboardData = {
      items: [
        {
          kind: 'file',
          type: file.type,
          getAsFile: () => file,
        },
      ],
      getData: () => '',
    };
    const paste = new Event('paste', { bubbles: true, cancelable: true });
    Object.defineProperty(paste, 'clipboardData', { value: clipboardData });
    target.dispatchEvent(paste);
  };

  const addClipboardImage = async (target) => {
    const invoke = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
    if (!invoke) return;
    let rid = null;
    try {
      rid = await invoke('plugin:clipboard-manager|read_image');
      if (rid == null) return;
      const rgba = new Uint8Array(await invoke('plugin:image|rgba', { rid }));
      const size = await invoke('plugin:image|size', { rid });
      if (!size || !size.width || !size.height || rgba.length === 0) return;

      const canvas = document.createElement('canvas');
      canvas.width = size.width;
      canvas.height = size.height;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      const imageData = ctx.createImageData(size.width, size.height);
      imageData.data.set(rgba);
      ctx.putImageData(imageData, 0, 0);

      const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
      if (!blob) return;
      const file = new File([blob], 'pasted-image.png', { type: 'image/png' });
      replayPasteWithImage(target, file);
    } catch (error) {
      // No image on the clipboard, or the platform cannot read one.
    } finally {
      if (rid != null) {
        invoke('plugin:resources|close', { rid }).catch(() => {});
      }
    }
  };

  document.addEventListener('paste', (event) => {
    if (hasNativeImageFile(event)) return;
    const target = event.target;
    const editable = target && (
      target.tagName === 'TEXTAREA' ||
      target.isContentEditable === true
    );
    if (!editable) return;
    addClipboardImage(target);
  }, true);
})();
"#;

fn build_window(app: &tauri::App, url: WebviewUrl, title: &str) -> tauri::Result<()> {
    let app_url = match &url {
        WebviewUrl::External(url) => Some(url.clone()),
        WebviewUrl::App(_) | _ => None,
    };

    let navigation_app_url = app_url.clone();
    let new_window_app_url = app_url;

    WebviewWindowBuilder::new(app, "main", url)
        .title(title)
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        .center()
        // Follow the desktop environment's current theme (None = system
        // settings), so scheduled / automatic light-dark switches keep
        // working while the window is open.
        .theme(None)
        // Allow the Web app to read the system clipboard (required on Linux
        // and Windows for pasting images and other rich clipboard content).
        .enable_clipboard_access()
        // Install the clipboard-image fallback for WebViews that do not put
        // clipboard images into the native paste event (mainly Linux).
        .initialization_script(PASTE_IMAGE_SCRIPT)
        // Regular same-tab navigations: keep the app origin in the shell,
        // open http(s)/mailto/tel links in the user's default application.
        .on_navigation(
            move |url| match link_action(url, navigation_app_url.as_ref()) {
                LinkAction::KeepInWebview => true,
                LinkAction::OpenExternally => {
                    open_externally(url);
                    false
                }
                LinkAction::Block => {
                    eprintln!("[dsh-desktop] blocked navigation to unsupported URL: {url}");
                    false
                }
            },
        )
        // window.open / target="_blank" requests: same-origin popups are
        // allowed for the Web app, external links are handed to the OS.
        .on_new_window(
            move |url, _features| match link_action(&url, new_window_app_url.as_ref()) {
                LinkAction::KeepInWebview => NewWindowResponse::Allow,
                LinkAction::OpenExternally => {
                    open_externally(&url);
                    NewWindowResponse::Deny
                }
                LinkAction::Block => {
                    eprintln!("[dsh-desktop] blocked unsupported new-window URL: {url}");
                    NewWindowResponse::Deny
                }
            },
        )
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
        .plugin(tauri_plugin_clipboard_manager::init())
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
        .run(|_app_handle, _event| {
            // With `theme(None)` the runtime follows the desktop
            // environment and propagates system theme changes to the
            // WebView automatically, so no manual theme handling is needed.
        });
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

    #[test]
    fn keeps_web_app_origin_in_the_shell() {
        let app_url = Url::parse("http://127.0.0.1:3080/chat").expect("app URL");
        for url in [
            "http://127.0.0.1:3080/chat/deep-think",
            "http://127.0.0.1:3080/api/models?detail=1",
        ] {
            let url = Url::parse(url).expect("internal URL");
            assert_eq!(link_action(&url, Some(&app_url)), LinkAction::KeepInWebview);
        }
    }

    #[test]
    fn opens_web_and_app_links_with_the_system_default_handler() {
        let app_url = Url::parse("http://127.0.0.1:3080").expect("app URL");
        for url in [
            "https://example.com/article",
            "http://127.0.0.1:3081/other-service",
            "mailto:user@example.com",
            "tel:+1234567890",
        ] {
            let url = Url::parse(url).expect("external URL");
            assert_eq!(
                link_action(&url, Some(&app_url)),
                LinkAction::OpenExternally
            );
        }
    }

    #[test]
    fn blocks_unsupported_link_schemes() {
        let app_url = Url::parse("http://127.0.0.1:3080").expect("app URL");
        for url in ["file:///etc/passwd", "data:text/plain,hello"] {
            let url = Url::parse(url).expect("unsupported URL");
            assert_eq!(link_action(&url, Some(&app_url)), LinkAction::Block);
        }
    }
}
