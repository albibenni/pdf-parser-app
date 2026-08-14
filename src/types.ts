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

export type QueueItem = {
  id: string;
  inputPath: string;
  status: "ready" | "converting" | "complete" | "error";
  error?: string;
  result?: ConversionResult;
};
