export type ConversionMode = "fast" | "balanced" | "text-only";

export type ConversionRequest = {
  inputPath: string;
  outputDir: string;
  mode: ConversionMode;
};

export type ConversionResult = {
  markdownPath: string;
  output: string;
};

export type RuntimeStatus = {
  state: "ready" | "missing";
  detail: string;
  markerInstalled: boolean;
  llamaCppInstalled: boolean;
};

export type InstallProgress = {
  currentBytes: number;
  totalBytes: number;
  bytesPerSecond: number;
  etaSeconds: number | null;
};

export type ConversionProgress = {
  inputPath: string;
  current: number;
  total: number | null;
  detail: string;
};

export type QueueItem = {
  id: string;
  inputPath: string;
  status: "ready" | "converting" | "complete" | "error";
  error?: string;
  result?: ConversionResult;
  progress?: ConversionProgress;
};
