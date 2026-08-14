use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
    if marker.is_file() {
        Ok(RuntimeStatus {
            state: "ready",
            detail: "Installed privately alongside PDF Parser.".to_string(),
        })
    } else {
        Ok(RuntimeStatus {
            state: "missing",
            detail: "Install Marker once; its Python environment and models stay in this app's data folder."
                .to_string(),
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

    let output = Command::new(&python)
        .args(["-m", "pip", "install", "--upgrade", "marker-pdf"])
        .output()
        .map_err(|error| format!("Could not install Marker: {error}"))?;
    ensure_success("install Marker", output)?;

    runtime_status(app)
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

    let model_cache = runtime_root.join("models");
    let mut command = Command::new(runtime_marker);
    command.arg(input).arg("--output_dir").arg(output_dir);
    command
        .env("HF_HOME", &model_cache)
        .env("HUGGINGFACE_HUB_CACHE", model_cache.join("hub"));
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

    let output = command
        .output()
        .map_err(|error| format!("Could not start the local Marker runtime: {error}"))?;
    let command_output = String::from_utf8_lossy(&output.stdout).to_string();
    let error_output = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(if error_output.is_empty() {
            format!("Marker exited with status {}.", output.status)
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
    use super::ConversionMode;

    #[test]
    fn deserializes_text_only_mode() {
        let mode: ConversionMode = serde_json::from_str("\"text-only\"").unwrap();
        assert!(matches!(mode, ConversionMode::TextOnly));
    }
}
