export interface HcaptchaWidgetConfig {
  callback: (token: string) => void;
  "expired-callback": () => void;
  "error-callback": () => void;
}

interface HcaptchaApi {
  render(container: HTMLElement, config: HcaptchaWidgetConfig & { sitekey: string }): string;
  reset(widgetId: string): void;
  remove(widgetId: string): void;
}

declare global {
  interface Window {
    hcaptcha?: HcaptchaApi;
  }
}

let scriptLoadPromise: Promise<void> | null = null;

export function hcaptchaSiteKey(): string | null {
  const value = import.meta.env.VITE_FILAMENT_HCAPTCHA_SITE_KEY;
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export async function ensureHcaptchaScript(): Promise<void> {
  if (window.hcaptcha) {
    return;
  }
  if (!scriptLoadPromise) {
    scriptLoadPromise = new Promise<void>((resolve, reject) => {
      const script = document.createElement("script");
      script.src = "https://js.hcaptcha.com/1/api.js?render=explicit";
      script.async = true;
      script.defer = true;
      script.onload = () => resolve();
      script.onerror = () => reject(new Error("Failed to load hCaptcha script."));
      document.head.appendChild(script);
    });
  }
  await scriptLoadPromise;
}
