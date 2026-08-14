import { describe, expect, it } from "vitest";
import { fileName, fileStem } from "./paths";

describe("path labels", () => {
  it("handles macOS and Linux path separators", () => {
    expect(fileName("/Users/albi/report.pdf")).toBe("report.pdf");
    expect(fileName("C:\\docs\\report.pdf")).toBe("report.pdf");
  });

  it("removes only the PDF extension", () => {
    expect(fileStem("/docs/project.v2.PDF")).toBe("project.v2");
  });
});
