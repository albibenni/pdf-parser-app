use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversionRequest {
    input_path: String,
    output_dir: String,
    mode: ConversionMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ConversionMode {
    Fast,
    Balanced,
    TextOnly,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionResult {
    markdown_path: String,
    output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    state: &'static str,
    detail: String,
    marker_installed: bool,
    llama_cpp_installed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    current_bytes: u64,
    total_bytes: u64,
    bytes_per_second: f64,
    eta_seconds: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionProgress {
    input_path: String,
    current: u64,
    total: Option<u64>,
    detail: String,
}

#[tauri::command]
async fn convert_pdf(
    app: AppHandle,
    request: ConversionRequest,
) -> Result<ConversionResult, String> {
    tauri::async_runtime::spawn_blocking(move || convert_with_marker(&app, request))
        .await
        .map_err(|error| format!("Conversion task failed: {error}"))?
}

#[tauri::command]
fn get_runtime_status(app: AppHandle) -> Result<RuntimeStatus, String> {
    runtime_status(&app)
}

#[tauri::command]
async fn install_marker_runtime(app: AppHandle) -> Result<RuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || install_runtime(&app))
        .await
        .map_err(|error| format!("Marker setup task failed: {error}"))?
}

fn runtime_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("runtime"))
        .map_err(|error| format!("Could not locate the app data folder: {error}"))
}

fn marker_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(runtime_root(app)?.join("venv/bin/marker_single"))
}

fn runtime_status(app: &AppHandle) -> Result<RuntimeStatus, String> {
    let marker = marker_path(app)?;
    let marker_installed = marker.is_file();
    let llama_cpp_installed = llama_server_path().is_some();

    if marker_installed && llama_cpp_installed {
        Ok(RuntimeStatus {
            state: "ready",
            detail: "Marker and llama.cpp are ready for local OCR conversion.".to_string(),
            marker_installed,
            llama_cpp_installed,
        })
    } else if marker_installed {
        Ok(RuntimeStatus {
            state: "missing",
            detail: "Marker is installed, but llama.cpp is needed for OCR conversion. Select Install llama.cpp to finish setup."
                .to_string(),
            marker_installed,
            llama_cpp_installed,
        })
    } else {
        Ok(RuntimeStatus {
            state: "missing",
            detail: "Install Marker once; its Python environment and models stay in this app's data folder."
                .to_string(),
            marker_installed,
            llama_cpp_installed,
        })
    }
}

fn install_runtime(app: &AppHandle) -> Result<RuntimeStatus, String> {
    let root = runtime_root(app)?;
    let venv = root.join("venv");
    let python = venv.join("bin/python");

    fs::create_dir_all(&root)
        .map_err(|error| format!("Could not create the Marker runtime folder: {error}"))?;

    if !python.is_file() {
        let output = Command::new("python3")
            .arg("-m")
            .arg("venv")
            .arg(&venv)
            .output()
            .map_err(|error| {
                format!(
                    "Python 3.10+ is required to set up Marker. Could not start python3: {error}"
                )
            })?;
        ensure_success("create the private Python environment", output)?;
    }

    install_marker_with_progress(app, &python)?;
    install_llama_cpp()?;

    runtime_status(app)
}

fn llama_server_path() -> Option<PathBuf> {
    let known_paths = [
        "/opt/homebrew/bin/llama-server",
        "/usr/local/bin/llama-server",
        "/usr/bin/llama-server",
    ];
    known_paths
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .or_else(|| command_path("llama-server"))
}

fn command_path(command: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(command)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!path.is_empty()).then_some(PathBuf::from(path))
        })
        .filter(|path| path.is_file())
}

fn brew_path() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .or_else(|| command_path("brew"))
}

fn install_llama_cpp() -> Result<(), String> {
    if llama_server_path().is_some() {
        return Ok(());
    }

    let output = if cfg!(target_os = "macos") {
        let brew = brew_path().ok_or_else(|| {
            "Homebrew is required to install llama.cpp on macOS. Install Homebrew, then retry."
                .to_string()
        })?;
        Command::new(brew)
            .args(["install", "llama.cpp"])
            .output()
            .map_err(|error| format!("Could not start Homebrew to install llama.cpp: {error}"))?
    } else if cfg!(target_os = "linux") {
        if command_path("pacman").is_none() {
            return Err(
                "Automatic llama.cpp setup on Linux currently supports Arch Linux only."
                    .to_string(),
            );
        }
        Command::new("pkexec")
            .args(["pacman", "-S", "--needed", "--noconfirm", "llama-cpp"])
            .output()
            .map_err(|error| {
                format!(
                    "Could not start the Arch Linux package installer. Install polkit, then retry: {error}"
                )
            })?
    } else {
        return Err(
            "llama.cpp automatic setup currently supports macOS and Arch Linux only.".to_string(),
        );
    };

    ensure_success("install llama.cpp", output)?;
    if llama_server_path().is_some() {
        Ok(())
    } else {
        Err(
            "llama.cpp installed but llama-server was not found. Restart PDF Parser and try again."
                .to_string(),
        )
    }
}

fn install_marker_with_progress(app: &AppHandle, python: &Path) -> Result<(), String> {
    let mut child = Command::new(python)
        .args([
            "-m",
            "pip",
            "install",
            "--upgrade",
            "--progress-bar",
            "raw",
            "marker-pdf",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start the Marker installer: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not read Marker installer progress.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not read Marker installer errors.".to_string())?;
    let stderr_reader = thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join("\n")
    });
    let mut last_bytes = 0_u64;
    let mut last_update = Instant::now();

    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some((current_bytes, total_bytes)) = parse_pip_progress(&line) {
            let elapsed = last_update.elapsed().as_secs_f64();
            let bytes_per_second = if current_bytes >= last_bytes && elapsed > 0.0 {
                (current_bytes - last_bytes) as f64 / elapsed
            } else {
                0.0
            };
            let eta_seconds = (bytes_per_second > 0.0 && total_bytes >= current_bytes)
                .then(|| (total_bytes - current_bytes) as f64 / bytes_per_second);
            let _ = app.emit(
                "marker-install-progress",
                InstallProgress {
                    current_bytes,
                    total_bytes,
                    bytes_per_second,
                    eta_seconds,
                },
            );
            last_bytes = current_bytes;
            last_update = Instant::now();
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("Could not wait for the Marker installer: {error}"))?;
    let error_output = stderr_reader
        .join()
        .map_err(|_| "Could not collect Marker installer errors.".to_string())?;
    if status.success() {
        Ok(())
    } else if error_output.is_empty() {
        Err(format!(
            "Could not install Marker; process exited with status {status}."
        ))
    } else {
        Err(format!("Could not install Marker: {error_output}"))
    }
}

fn parse_pip_progress(line: &str) -> Option<(u64, u64)> {
    let values = line.strip_prefix("Progress ")?.split(" of ");
    let mut values = values.map(str::parse::<u64>);
    match (values.next()?.ok()?, values.next()?.ok()?) {
        (current, total) if total > 0 => Some((current, total)),
        _ => None,
    }
}

fn ensure_success(action: &str, output: std::process::Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let error_output = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if error_output.is_empty() {
        format!(
            "Could not {action}; process exited with status {}.",
            output.status
        )
    } else {
        format!("Could not {action}: {error_output}")
    })
}

fn convert_with_marker(
    app: &AppHandle,
    request: ConversionRequest,
) -> Result<ConversionResult, String> {
    let input = Path::new(&request.input_path);
    let output_dir = Path::new(&request.output_dir);

    if input.extension().is_none_or(|extension| extension != "pdf") {
        return Err("Only PDF files can be converted.".to_string());
    }
    if !input.is_file() {
        return Err(format!("PDF not found: {}", input.display()));
    }
    if !output_dir.is_dir() {
        return Err(format!("Output folder not found: {}", output_dir.display()));
    }

    let runtime_root = runtime_root(app)?;
    let runtime_marker = marker_path(app)?;
    if !runtime_marker.is_file() {
        return Err(
            "Marker is not installed. Select Install Marker in PDF Parser first.".to_string(),
        );
    }
    let llama_server = llama_server_path().ok_or_else(|| {
        "llama.cpp is not installed. Select Install llama.cpp in PDF Parser before using OCR modes."
            .to_string()
    })?;

    let model_cache = runtime_root.join("models");
    let mut command = Command::new(runtime_marker);
    command.arg(input).arg("--output_dir").arg(output_dir);
    command
        .env("HF_HOME", &model_cache)
        .env("HUGGINGFACE_HUB_CACHE", model_cache.join("hub"))
        .env("LLAMA_CPP_BINARY", llama_server);
    match request.mode {
        ConversionMode::Fast => {
            command.arg("--mode").arg("fast");
        }
        ConversionMode::Balanced => {
            command.arg("--mode").arg("balanced");
        }
        ConversionMode::TextOnly => {
            command.arg("--mode").arg("fast").arg("--disable_ocr");
        }
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start the local Marker runtime: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not read Marker output.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not read Marker progress.".to_string())?;
    let progress_app = app.clone();
    let progress_input = request.input_path.clone();
    let stderr_reader =
        thread::spawn(move || read_marker_stderr(stderr, progress_app, progress_input));
    let command_output = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .collect::<Vec<_>>()
        .join("\n");
    let status = child
        .wait()
        .map_err(|error| format!("Could not wait for the local Marker runtime: {error}"))?;
    let error_output = stderr_reader
        .join()
        .map_err(|_| "Could not collect Marker output.".to_string())?
        .trim()
        .to_string();

    if !status.success() {
        return Err(if error_output.is_empty() {
            format!("Marker exited with status {status}.")
        } else {
            format!("Marker failed: {error_output}")
        });
    }

    let file_name = input
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Could not determine the PDF file name.".to_string())?;
    let markdown_path = output_dir.join(file_name).join(format!("{file_name}.md"));

    Ok(ConversionResult {
        markdown_path: markdown_path.to_string_lossy().to_string(),
        output: command_output,
    })
}

fn read_marker_stderr(stderr: impl std::io::Read, app: AppHandle, input_path: String) -> String {
    let mut reader = BufReader::new(stderr);
    let mut output = String::new();
    let mut buffer = Vec::new();

    while reader.read_until(b'\r', &mut buffer).unwrap_or(0) > 0 {
        let line = String::from_utf8_lossy(&buffer);
        if let Some((current, total)) = parse_marker_progress(&line) {
            let _ = app.emit(
                "marker-conversion-progress",
                ConversionProgress {
                    input_path: input_path.clone(),
                    current,
                    total: Some(total),
                    detail: "Converting document".to_string(),
                },
            );
        }
        output.push_str(&line);
        buffer.clear();
    }

    output
}

fn parse_marker_progress(line: &str) -> Option<(u64, u64)> {
    line.split_whitespace().rev().find_map(|token| {
        let token =
            token.trim_matches(|character: char| !character.is_ascii_digit() && character != '/');
        let (current, total) = token.split_once('/')?;
        let current = current.parse().ok()?;
        let total = total.parse().ok()?;
        (total > 0 && current <= total).then_some((current, total))
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            convert_pdf,
            get_runtime_status,
            install_marker_runtime
        ])
        .run(tauri::generate_context!())
        .expect("error while running PDF Parser");
}

#[cfg(test)]
mod tests {
    use super::{parse_marker_progress, parse_pip_progress, ConversionMode};

    #[test]
    fn deserializes_text_only_mode() {
        let mode: ConversionMode = serde_json::from_str("\"text-only\"").unwrap();
        assert!(matches!(mode, ConversionMode::TextOnly));
    }

    #[test]
    fn parses_pip_raw_progress() {
        assert_eq!(
            parse_pip_progress("Progress 500 of 1000"),
            Some((500, 1000))
        );
        assert_eq!(parse_pip_progress("Progress 500 of 0"), None);
    }

    #[test]
    fn parses_marker_progress() {
        assert_eq!(
            parse_marker_progress("Converting:  50%|█████| 3/6 [00:01<00:01]"),
            Some((3, 6))
        );
    }
}
