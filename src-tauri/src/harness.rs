//! Spawning and supervising the DeepSeek Harness runtime process.
//!
//! The shell owns one harness child process and opens the Web surface whose
//! URL the child prints on stdout (dsh web: http://127.0.0.1:<port>). The
//! child inherits stderr so boot failures stay visible on the launching
//! terminal. DSH_BIN/DSH_ARGS select the command (default dsh web), DSH_PORT
//! appends a --port argument, and the shell always forces --host 127.0.0.1
//! so the published URL is loopback.
//!
//! Shutdown sends SIGTERM first and escalates to SIGKILL after a grace
//! window, so the harness drains (session flush, terminal restore) before
//! the shell exits.

use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

/// How long the shell waits for the harness to publish its URL.
const URL_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a SIGTERM'd harness may keep draining before SIGKILL.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// A running harness process plus the loopback URL it published.
#[derive(Debug)]
pub struct SpawnedHarness {
    /// The owned child; dropping it does not kill the process.
    pub child: Child,
    /// The published loopback Web URL.
    pub url: Url,
}

impl SpawnedHarness {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

/// The spawn command: DSH_BIN plus whitespace-split DSH_ARGS, then
/// --host 127.0.0.1 and, when DSH_PORT is set, --port <port>.
pub fn command_from_env() -> Vec<String> {
    let mut command = vec![env::var("DSH_BIN").unwrap_or_else(|_| String::from("dsh"))];
    command.extend(
        env::var("DSH_ARGS")
            .unwrap_or_else(|_| String::from("web"))
            .split_whitespace()
            .map(str::to_string),
    );
    command.push(String::from("--host"));
    command.push(String::from("127.0.0.1"));
    if let Ok(port) = env::var("DSH_PORT") {
        command.push(String::from("--port"));
        command.push(port);
    }
    command
}

/// Extracts the first loopback URL with a port from a printed line, e.g.
/// dsh web: http://127.0.0.1:3080 (LAN: http://192.168.1.5:3080).
pub fn scan_loopback_url(line: &str) -> Option<Url> {
    for token in line.split_whitespace() {
        let Some(rest) = token.strip_prefix("http://") else {
            continue;
        };
        let candidate = format!(
            "http://{}",
            rest.trim_end_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '.' && c != ':' && c != '-' && c != '/'
            })
        );
        let Ok(url) = Url::parse(&candidate) else {
            continue;
        };
        let Some(host) = url.host_str() else {
            continue;
        };
        let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
        if loopback && url.port().is_some() {
            return Some(url);
        }
    }
    None
}

/// Spawns the harness and blocks until it publishes its loopback URL or the
/// process exits. Lines from the child's stdout are echoed with a [dsh]
/// prefix.
pub fn spawn_and_wait() -> Result<SpawnedHarness, String> {
    let command = command_from_env();
    let (bin, args) = command
        .split_first()
        .ok_or_else(|| String::from("DSH_BIN is empty"))?;
    let mut child = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to start {bin}: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| String::from("harness stdout unavailable"))?;
    let (_reader, lines) = forward_lines(stdout);
    let deadline = Instant::now() + URL_WAIT_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!(
                "harness exited with {status} before publishing its URL"
            ));
        }
        match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                if let Some(url) = scan_loopback_url(&line) {
                    return Ok(SpawnedHarness { child, url });
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait().map_err(|error| error.to_string())?;
                return Err(format!(
                    "harness exited with {status} before publishing its URL"
                ));
            }
        }
    }
    Err(String::from(
        "timed out waiting for the harness to publish its URL",
    ))
}

/// Echoes every line read from stdout with a [dsh] prefix and forwards it to
/// the returned receiver. The reader thread ends when the stream closes.
fn forward_lines(
    stdout: impl std::io::Read + Send + 'static,
) -> (thread::JoinHandle<()>, Receiver<String>) {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("[dsh] {line}");
            let _ = sender.send(line);
        }
    });
    (handle, receiver)
}

/// Terminates the harness: SIGTERM first, SIGKILL after the grace window.
#[cfg(unix)]
pub fn terminate(pid: u32) {
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    let deadline = Instant::now() + SHUTDOWN_GRACE;
    loop {
        let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
        if !alive {
            return;
        }
        if Instant::now() >= deadline {
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Terminates the harness process tree on Windows.
#[cfg(not(unix))]
pub fn terminate(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    /// Serializes the environment-mutating and subprocess tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn scans_the_printed_web_url() {
        let line = "dsh web: http://127.0.0.1:3080 (LAN: http://192.168.1.5:3080)";
        let url = scan_loopback_url(line).expect("loopback URL");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(3080));
    }

    #[test]
    fn ignores_non_loopback_urls() {
        assert!(scan_loopback_url("(LAN: http://192.168.1.5:3080)").is_none());
        assert!(scan_loopback_url("nothing here").is_none());
        assert!(scan_loopback_url("http://127.0.0.1").is_none());
    }

    #[test]
    fn defaults_to_the_dsh_web_command() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("DSH_BIN");
        env::remove_var("DSH_ARGS");
        env::remove_var("DSH_PORT");
        assert_eq!(command_from_env(), ["dsh", "web", "--host", "127.0.0.1"]);
    }

    #[cfg(unix)]
    #[test]
    fn spawns_a_fake_harness_and_terminates_it() {
        let _guard = ENV_LOCK.lock().unwrap();
        let script = env::temp_dir().join(format!(
            "dsh-desktop-spawn-test-{}.sh",
            std::process::id()
        ));
        fs::write(
            &script,
            "#!/bin/sh\necho 'dsh web: http://127.0.0.1:4199'\nsleep 60\n",
        )
        .expect("write script");
        env::set_var("DSH_BIN", "sh");
        env::set_var("DSH_ARGS", script.to_string_lossy().to_string());
        env::remove_var("DSH_PORT");

        let spawned = spawn_and_wait().expect("spawn and wait");
        assert_eq!(spawned.url.port(), Some(4199));
        terminate(spawned.pid());
        // Reap the killed child so the liveness probe below sees ESRCH
        // instead of a zombie.
        let mut spawned = spawned;
        let status = spawned.child.wait().expect("reap child");
        assert!(!status.success(), "child survived terminate: {status}");
        assert_eq!(
            unsafe { libc::kill(spawned.pid() as i32, 0) },
            -1,
            "child still alive after terminate"
        );
        let _ = fs::remove_file(&script);
    }

    #[cfg(unix)]
    #[test]
    fn reports_a_harness_that_exits_first() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("DSH_BIN", "false");
        env::set_var("DSH_ARGS", "");
        env::remove_var("DSH_PORT");
        let error = spawn_and_wait().expect_err("harness that exits");
        assert!(error.contains("exited"), "unexpected error: {error}");
    }
}
