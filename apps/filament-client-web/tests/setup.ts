import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

vi.mock("emoji-mart", () => ({
  Picker: class MockPicker {
    constructor() {
      const div = document.createElement("div");
      div.setAttribute("data-testid", "mock-emoji-picker");
      return div;
    }
  },
  init: vi.fn(),
  SearchIndex: { search: vi.fn() },
}));

if (typeof window !== "undefined") {
  Object.defineProperty(window, "scrollTo", {
    writable: true,
    value: () => {},
  });
}
