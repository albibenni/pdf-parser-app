use std::{path::Path, process::Command};

use serde::{Deserialize, Serialize};

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

#[tauri::command]
async fn convert_pdf(request: ConversionRequest) -> Result<ConversionResult, String> {
    tauri::async_runtime::spawn_blocking(move || convert_with_marker(request))
        .await
        .map_err(|error| format!("Conversion task failed: {error}"))?
}

fn convert_with_marker(request: ConversionRequest) -> Result<ConversionResult, String> {
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

    let mut command = Command::new("marker_single");
    command.arg(input).arg("--output_dir").arg(output_dir);
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

    let output = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "Marker is not installed yet. Install the local Marker runtime, then restart PDF Parser."
                .to_string()
        } else {
            format!("Could not start Marker: {error}")
        }
    })?;
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
        .invoke_handler(tauri::generate_handler![convert_pdf])
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
