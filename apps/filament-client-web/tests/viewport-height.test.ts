import { describe, expect, it, vi } from "vitest";
import { installViewportHeightCssVar } from "../src/lib/viewport-height";

interface WindowLikeFixture {
  innerHeight: number;
  document: {
    documentElement: {
      style: {
        setProperty: ReturnType<typeof vi.fn>;
      };
    };
  };
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
}

function createWindowFixture(innerHeight = 720): WindowLikeFixture {
  return {
    innerHeight,
    document: {
      documentElement: {
        style: {
          setProperty: vi.fn(),
        },
      },
    },
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
}

describe("viewport height css var sync", () => {
  it("syncs css var immediately and on resize/orientation events", () => {
    const browserWindow = createWindowFixture(801.4);

    const dispose = installViewportHeightCssVar(browserWindow as any);

    expect(browserWindow.document.documentElement.style.setProperty).toHaveBeenCalledWith(
      "--app-viewport-height",
      "801px",
    );
    expect(browserWindow.addEventListener).toHaveBeenCalledWith(
      "resize",
      expect.any(Function),
    );
    expect(browserWindow.addEventListener).toHaveBeenCalledWith(
      "orientationchange",
      expect.any(Function),
    );

    const resizeListener = browserWindow.addEventListener.mock.calls.find(
      ([name]) => name === "resize",
    )?.[1];
    expect(typeof resizeListener).toBe("function");

    browserWindow.innerHeight = 900.1;
    resizeListener?.();

    expect(browserWindow.document.documentElement.style.setProperty).toHaveBeenLastCalledWith(
      "--app-viewport-height",
      "900px",
    );

    dispose();
    expect(browserWindow.removeEventListener).toHaveBeenCalledWith(
      "resize",
      resizeListener,
    );
  });

  it("returns noop when browser context is missing", () => {
    const dispose = installViewportHeightCssVar(null);
    expect(typeof dispose).toBe("function");
    expect(() => dispose()).not.toThrow();
  });

  it("ignores invalid viewport heights", () => {
    const browserWindow = createWindowFixture(0);

    installViewportHeightCssVar(browserWindow as any);

    expect(browserWindow.document.documentElement.style.setProperty).not.toHaveBeenCalled();
  });
});
