import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { uriToPath } from "./lsp";

describe("uriToPath", () => {
  it("strips file:// from POSIX paths", () => {
    expect(uriToPath("file:///home/user/foo.cpp")).toBe("/home/user/foo.cpp");
  });

  it("converts Windows file:///c:/foo to drive-letter form", () => {
    expect(uriToPath("file:///c:/Users/alice/main.cpp")).toBe("c:/Users/alice/main.cpp");
  });

  it("preserves uppercase Windows drive letters", () => {
    expect(uriToPath("file:///C:/proj/src/lib.rs")).toBe("C:/proj/src/lib.rs");
  });

  it("decodes percent-encoded segments", () => {
    expect(uriToPath("file:///home/user/My%20Project/main.cpp")).toBe(
      "/home/user/My Project/main.cpp",
    );
  });

  it("returns the input unchanged when no file:// prefix", () => {
    expect(uriToPath("https://example.com/x")).toBe("https://example.com/x");
  });
});
