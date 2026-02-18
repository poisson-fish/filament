const VIEWPORT_HEIGHT_CSS_VAR = "--app-viewport-height";

interface CssStyleDeclarationLike {
  setProperty(name: string, value: string): void;
}

interface DocumentElementLike {
  style?: CssStyleDeclarationLike | null;
}

interface DocumentLike {
  documentElement?: DocumentElementLike | null;
}

interface EventTargetLike {
  addEventListener(
    type: "resize" | "orientationchange",
    listener: () => void,
  ): void;
  removeEventListener(
    type: "resize" | "orientationchange",
    listener: () => void,
  ): void;
}

interface BrowserWindowLike extends EventTargetLike {
  innerHeight: number;
  document?: DocumentLike | null;
}

function roundedViewportHeightPx(innerHeight: number): string | null {
  if (!Number.isFinite(innerHeight) || innerHeight <= 0) {
    return null;
  }
  return `${Math.round(innerHeight)}px`;
}

export function installViewportHeightCssVar(
  browserWindow: BrowserWindowLike | null | undefined,
): () => void {
  const rootStyle = browserWindow?.document?.documentElement?.style;
  if (!browserWindow || !rootStyle || typeof rootStyle.setProperty !== "function") {
    return () => {};
  }

  const syncCssVar = (): void => {
    const height = roundedViewportHeightPx(browserWindow.innerHeight);
    if (!height) {
      return;
    }
    rootStyle.setProperty(VIEWPORT_HEIGHT_CSS_VAR, height);
  };

  syncCssVar();
  browserWindow.addEventListener("resize", syncCssVar);
  browserWindow.addEventListener("orientationchange", syncCssVar);

  return () => {
    browserWindow.removeEventListener("resize", syncCssVar);
    browserWindow.removeEventListener("orientationchange", syncCssVar);
  };
}
