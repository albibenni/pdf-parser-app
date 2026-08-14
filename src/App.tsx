import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useMemo, useState } from "react";
import { fileName, fileStem } from "./lib/paths";
import type {
  ConversionMode,
  ConversionRequest,
  ConversionResult,
  QueueItem,
} from "./types";

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

  const readyCount = useMemo(
    () => queue.filter((item) => item.status === "ready").length,
    [queue],
  );

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
    if (!outputDir) {
      setMessage("Choose an output folder first.");
      return;
    }
    if (readyCount === 0) return;

    for (const item of queue.filter((entry) => entry.status === "ready")) {
      setQueue((current) =>
        current.map((entry) =>
          entry.id === item.id ? { ...entry, status: "converting" } : entry,
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
        <button className="primary-button" type="button" onClick={choosePdfs}>
          Add PDFs
        </button>
      </section>

      <section className="settings-grid">
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

        <article className="card setting-card">
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
      </section>

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
            disabled={readyCount === 0}
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
              </div>
              <span className={`status status-${item.status}`}>
                {item.status}
              </span>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}

export default App;
