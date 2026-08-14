import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import { fileName, fileStem } from "./lib/paths";
import type {
  ConversionMode,
  ConversionProgress,
  ConversionRequest,
  ConversionResult,
  InstallProgress,
  QueueItem,
  RuntimeStatus,
} from "./types";

const formatBytes = (bytes: number) => {
  const units = ["B", "KB", "MB", "GB"];
  const unit = Math.min(Math.floor(Math.log10(Math.max(bytes, 1)) / 3), 3);
  return `${(bytes / 1000 ** unit).toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
};

const formatEta = (seconds: number | null) => {
  if (seconds === null || !Number.isFinite(seconds)) return "calculating…";
  return `~${Math.max(1, Math.ceil(seconds))}s left`;
};

const modeDescription: Record<ConversionMode, string> = {
  fast: "Best default for M-series Macs and CPU-only Linux.",
  balanced: "More OCR and layout work for difficult documents.",
  "text-only": "Fastest; skips OCR and does not handle scans or equations.",
};

const createItem = (inputPath: string): QueueItem => ({
  id: crypto.randomUUID(),
  inputPath,
  status: "ready",
});

function App() {
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [mode, setMode] = useState<ConversionMode>("fast");
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [message, setMessage] = useState(
    "Choose PDFs to begin. Files stay on this device.",
  );
  const [runtime, setRuntime] = useState<RuntimeStatus>({
    state: "missing",
    detail: "Checking the local Marker runtime…",
    markerInstalled: false,
    llamaCppInstalled: false,
  });
  const [installingRuntime, setInstallingRuntime] = useState(false);
  const [installProgress, setInstallProgress] =
    useState<InstallProgress | null>(null);

  const readyCount = useMemo(
    () => queue.filter((item) => item.status === "ready").length,
    [queue],
  );

  const refreshRuntime = useCallback(async () => {
    const status = await invoke<RuntimeStatus>("get_runtime_status");
    setRuntime(status);
  }, []);

  useEffect(() => {
    void refreshRuntime().catch((error) =>
      setRuntime({
        state: "missing",
        detail: String(error),
        markerInstalled: false,
        llamaCppInstalled: false,
      }),
    );
  }, [refreshRuntime]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<InstallProgress>("marker-install-progress", (event) => {
      setInstallProgress(event.payload);
    }).then((remove) => {
      unlisten = remove;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<ConversionProgress>("marker-conversion-progress", (event) => {
      setQueue((current) =>
        current.map((item) =>
          item.inputPath === event.payload.inputPath
            ? { ...item, progress: event.payload }
            : item,
        ),
      );
    }).then((remove) => {
      unlisten = remove;
    });
    return () => unlisten?.();
  }, []);

  const installRuntime = async () => {
    setInstallingRuntime(true);
    setInstallProgress(null);
    setMessage(
      "Installing local conversion tools. This can take a few minutes.",
    );
    try {
      const status = await invoke<RuntimeStatus>("install_marker_runtime");
      setRuntime(status);
      setMessage("Marker and llama.cpp are ready for local conversion.");
    } catch (error) {
      const detail = String(error);
      setRuntime({
        state: "missing",
        detail,
        markerInstalled: runtime.markerInstalled,
        llamaCppInstalled: runtime.llamaCppInstalled,
      });
      setMessage("Marker setup did not finish.");
    } finally {
      setInstallingRuntime(false);
    }
  };

  const choosePdfs = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    setQueue((current) => [...current, ...paths.map(createItem)]);
    setMessage(`${paths.length} PDF${paths.length === 1 ? "" : "s"} added.`);
  };

  const chooseOutput = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    setOutputDir(selected);
  };

  const runQueue = async () => {
    if (runtime.state !== "ready") {
      setMessage("Install the local Marker runtime before converting.");
      return;
    }
    if (!outputDir) {
      setMessage("Choose an output folder first.");
      return;
    }
    if (readyCount === 0) return;

    for (const item of queue.filter((entry) => entry.status === "ready")) {
      setQueue((current) =>
        current.map((entry) =>
          entry.id === item.id
            ? {
                ...entry,
                status: "converting",
                progress: {
                  inputPath: item.inputPath,
                  current: 0,
                  total: null,
                  detail: "Starting conversion…",
                },
              }
            : entry,
        ),
      );
      try {
        const request: ConversionRequest = {
          inputPath: item.inputPath,
          outputDir,
          mode,
        };
        const result = await invoke<ConversionResult>("convert_pdf", {
          request,
        });
        setQueue((current) =>
          current.map((entry) =>
            entry.id === item.id
              ? { ...entry, status: "complete", result }
              : entry,
          ),
        );
      } catch (error) {
        setQueue((current) =>
          current.map((entry) =>
            entry.id === item.id
              ? { ...entry, status: "error", error: String(error) }
              : entry,
          ),
        );
      }
    }
    setMessage("Conversion queue finished.");
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">LOCAL-FIRST CONVERSION</p>
          <h1>PDF Parser</h1>
        </div>
        <span className="local-badge">● Files never leave your device</span>
      </header>

      <section className="hero card">
        <div>
          <p className="eyebrow">MARKER BACKEND</p>
          <h2>Turn PDFs into useful Markdown.</h2>
          <p>
            Extract document structure, tables, equations, and images into a
            Markdown folder you control.
          </p>
        </div>
      </section>

      <section className="settings-grid">
        <article className="card setting-card runtime-card">
          <p className="eyebrow">LOCAL RUNTIME</p>
          <h2>
            {runtime.state === "ready"
              ? "OCR conversion is ready"
              : "Set up OCR"}
          </h2>
          <p>{runtime.detail}</p>
          {installingRuntime && (
            <>
              <div
                aria-label="Installing Marker"
                aria-valuemax={100}
                aria-valuemin={0}
                aria-valuenow={
                  installProgress
                    ? Math.round(
                        (installProgress.currentBytes /
                          installProgress.totalBytes) *
                          100,
                      )
                    : undefined
                }
                aria-valuetext={
                  installProgress
                    ? `${formatBytes(installProgress.currentBytes)} of ${formatBytes(installProgress.totalBytes)}`
                    : "Preparing installation"
                }
                className="install-progress"
                role="progressbar"
              >
                <span
                  style={{
                    width: installProgress
                      ? `${(installProgress.currentBytes / installProgress.totalBytes) * 100}%`
                      : undefined,
                  }}
                />
              </div>
              <p className="install-details">
                {installProgress
                  ? `${formatBytes(installProgress.currentBytes)} / ${formatBytes(installProgress.totalBytes)} · ${formatBytes(installProgress.bytesPerSecond)}/s · ${formatEta(installProgress.etaSeconds)}`
                  : "Preparing the private Python environment…"}
              </p>
            </>
          )}
          {runtime.state === "missing" && (
            <button
              className="secondary-button"
              type="button"
              disabled={installingRuntime}
              onClick={() => void installRuntime()}
            >
              {installingRuntime
                ? "Installing conversion tools…"
                : runtime.markerInstalled
                  ? "Install llama.cpp"
                  : runtime.llamaCppInstalled
                    ? "Install Marker"
                    : "Install Marker and llama.cpp"}
            </button>
          )}
        </article>

        <article className="card setting-card">
          <p className="eyebrow">INPUT PDFS</p>
          <h2>{queue.length ? `${queue.length} selected` : "Choose PDFs"}</h2>
          <p>PDFs are converted locally and remain in their original folder.</p>
          <button className="primary-button" type="button" onClick={choosePdfs}>
            Add PDFs
          </button>
        </article>

        <article className="card setting-card">
          <p className="eyebrow">OUTPUT</p>
          <h2>{outputDir ? fileName(outputDir) : "Choose a folder"}</h2>
          <p className="path">{outputDir ?? "No output folder selected"}</p>
          <button
            className="secondary-button"
            type="button"
            onClick={chooseOutput}
          >
            {outputDir ? "Change folder" : "Select output folder"}
          </button>
        </article>
      </section>

      <section className="conversion-row">
        <article className="card setting-card mode-card">
          <label className="eyebrow" htmlFor="conversion-mode">
            CONVERSION MODE
          </label>
          <select
            id="conversion-mode"
            value={mode}
            onChange={(event) => setMode(event.target.value as ConversionMode)}
          >
            <option value="fast">Fast</option>
            <option value="balanced">Balanced</option>
            <option value="text-only">Text only</option>
          </select>
          <p>{modeDescription[mode]}</p>
        </article>

        <section className="queue-section card">
          <div className="queue-header">
            <div>
              <p className="eyebrow">QUEUE</p>
              <h2>
                {queue.length ? `${queue.length} files` : "No PDFs selected"}
              </h2>
            </div>
            <button
              className="primary-button"
              type="button"
              disabled={readyCount === 0 || runtime.state !== "ready"}
              onClick={() => void runQueue()}
            >
              Convert {readyCount || ""} to Markdown
            </button>
          </div>
          <p className="status-message">{message}</p>
          <ul className="queue-list">
            {queue.map((item) => (
              <li key={item.id} className="queue-item">
                <div>
                  <strong>{fileName(item.inputPath)}</strong>
                  <span className="path">{item.inputPath}</span>
                  {item.result && (
                    <span className="path">
                      → {fileStem(item.result.markdownPath)}.md
                    </span>
                  )}
                  {item.error && <span className="error">{item.error}</span>}
                  {item.status === "converting" && (
                    <>
                      <div
                        aria-label={`Conversion progress for ${fileName(item.inputPath)}`}
                        aria-valuemax={item.progress?.total ?? undefined}
                        aria-valuemin={0}
                        aria-valuenow={item.progress?.current || undefined}
                        aria-valuetext={
                          item.progress?.total
                            ? `${item.progress.current} of ${item.progress.total}`
                            : (item.progress?.detail ?? "Starting conversion")
                        }
                        className="conversion-progress"
                        role="progressbar"
                      >
                        <span
                          style={{
                            width: item.progress?.total
                              ? `${Math.min(100, (item.progress.current / item.progress.total) * 100)}%`
                              : undefined,
                          }}
                        />
                      </div>
                      <span className="conversion-details">
                        {item.progress?.total
                          ? `${item.progress.current} of ${item.progress.total} steps`
                          : (item.progress?.detail ?? "Starting conversion…")}
                      </span>
                    </>
                  )}
                </div>
                <span className={`status status-${item.status}`}>
                  {item.status}
                </span>
              </li>
            ))}
          </ul>
        </section>
      </section>
    </main>
  );
}

export default App;
