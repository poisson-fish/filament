import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import { Navigate, useNavigate } from "@solidjs/router";
import {
  captchaTokenFromInput,
  DomainValidationError,
  passwordFromInput,
  usernameFromInput,
} from "../domain/auth";
import { ApiError, loginWithPassword, registerWithPassword } from "../lib/api";
import { useAuth } from "../lib/auth-context";
import { ensureHcaptchaScript, hcaptchaSiteKey } from "../lib/hcaptcha";

function mapApiError(error: unknown): string {
  if (error instanceof DomainValidationError) {
    return error.message;
  }
  if (error instanceof ApiError) {
    if (error.code === "invalid_credentials") {
      return "Invalid credentials.";
    }
    if (error.code === "rate_limited") {
      return "Too many auth requests. Please wait and retry.";
    }
    if (error.code === "network_error") {
      return "Cannot reach server. Verify API origin and TLS setup.";
    }
    if (error.code === "captcha_failed") {
      return "Captcha verification failed. Please retry.";
    }
    return "Auth failed. Please retry.";
  }
  return "Unexpected error. Please retry.";
}

export function LoginPage() {
  const auth = useAuth();
  const navigate = useNavigate();
  if (auth.session()) {
    return <Navigate href="/app" />;
  }
  const [isRegisterMode, setRegisterMode] = createSignal(false);
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [isSubmitting, setSubmitting] = createSignal(false);
  const [statusMessage, setStatusMessage] = createSignal("");
  const [errorMessage, setErrorMessage] = createSignal("");
  const [loginCaptchaRequired, setLoginCaptchaRequired] = createSignal(false);
  const [captchaReady, setCaptchaReady] = createSignal(false);
  const [captchaToken, setCaptchaToken] = createSignal("");
  const [captchaError, setCaptchaError] = createSignal("");
  let captchaContainer: HTMLDivElement | undefined;
  let captchaWidgetId: string | null = null;
  const siteKey = hcaptchaSiteKey();
  const shouldRenderCaptcha = () =>
    siteKey !== null && (isRegisterMode() || loginCaptchaRequired());

  const resetCaptcha = () => {
    setCaptchaToken("");
    if (captchaWidgetId && window.hcaptcha) {
      window.hcaptcha.reset(captchaWidgetId);
    }
  };

  createEffect(() => {
    if (!shouldRenderCaptcha() || !captchaContainer || captchaWidgetId || !siteKey) {
      return;
    }
    void (async () => {
      try {
        await ensureHcaptchaScript();
        if (!window.hcaptcha || !captchaContainer) {
          throw new Error("hcaptcha unavailable");
        }
        captchaWidgetId = window.hcaptcha.render(captchaContainer, {
          sitekey: siteKey,
          callback: (token) => {
            setCaptchaToken(token);
            setCaptchaError("");
          },
          "expired-callback": () => {
            setCaptchaToken("");
            setCaptchaError("Captcha expired. Please verify again.");
          },
          "error-callback": () => {
            setCaptchaToken("");
            setCaptchaError("Captcha failed to load. Please retry.");
          },
        });
        setCaptchaReady(true);
      } catch {
        setCaptchaReady(false);
        setCaptchaError("Captcha unavailable. Please retry later.");
      }
    })();
  });

  onCleanup(() => {
    if (captchaWidgetId && window.hcaptcha) {
      window.hcaptcha.remove(captchaWidgetId);
    }
  });

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (isSubmitting()) {
      return;
    }

    setErrorMessage("");
    setStatusMessage("");
    setSubmitting(true);

    try {
      const validatedUsername = usernameFromInput(username().trim());
      const validatedPassword = passwordFromInput(password());
      const validatedCaptchaToken =
        shouldRenderCaptcha() ? captchaTokenFromInput(captchaToken()) : undefined;

      if (isRegisterMode()) {
        await registerWithPassword({
          username: validatedUsername,
          password: validatedPassword,
          captchaToken: validatedCaptchaToken,
        });
        setStatusMessage("Account accepted. Continue with login.");
        setRegisterMode(false);
        resetCaptcha();
      } else {
        const session = await loginWithPassword({
          username: validatedUsername,
          password: validatedPassword,
          captchaToken: validatedCaptchaToken,
        });
        auth.setAuthenticatedSession(session);
        navigate("/app", { replace: true });
      }
    } catch (error) {
      if (error instanceof ApiError && error.code === "captcha_failed") {
        if (!isRegisterMode()) {
          setLoginCaptchaRequired(true);
          if (siteKey === null) {
            setCaptchaError("Captcha is required by server but site key is missing in web config.");
          }
        }
        resetCaptcha();
      }
      setErrorMessage(mapApiError(error));
    } finally {
      setSubmitting(false);
    }
  };

  const authModeButtonClass = (active: boolean): string =>
    [
      "cursor-pointer rounded-[0.7rem] border px-[0.72rem] py-[0.62rem] transition-[transform,background-color] duration-[120ms] ease-out",
      active
        ? "border-brand bg-gradient-to-b from-brand to-brand-strong text-white"
        : "border-line-soft bg-bg-3 text-ink-1 hover:-translate-y-px",
    ].join(" ");

  return (
    <div class="auth-layout grid min-h-screen place-items-center p-[1.4rem]">
      <div class="auth-panel fx-panel w-full max-w-[29rem] rounded-[1.1rem] p-[1.5rem] shadow-[0_1.2rem_3rem_var(--shadow)]">
        <header class="auth-header">
          <p class="m-0 text-[0.76rem] tracking-[0.14em] text-ink-2 uppercase">Filament</p>
          <h1 class="mb-0 mt-[0.25rem] tracking-[0.015em]">
            {isRegisterMode() ? "Create Account" : "Welcome Back"}
          </h1>
          <p class="mb-0 mt-[0.4rem] text-ink-2">
            {isRegisterMode()
              ? "Register with a valid username and strong password."
              : "Login to enter your workspace."}
          </p>
        </header>

        <div
          class="auth-mode-switch mt-[1.1rem] grid grid-cols-2 gap-[0.5rem]"
          role="tablist"
          aria-label="Authentication mode"
        >
          <button
            type="button"
            class={authModeButtonClass(!isRegisterMode())}
            onClick={() => {
              setRegisterMode(false);
              setCaptchaError("");
            }}
          >
            Login
          </button>
          <button
            type="button"
            class={authModeButtonClass(isRegisterMode())}
            onClick={() => {
              setRegisterMode(true);
              setCaptchaError("");
            }}
          >
            Register
          </button>
        </div>

        <form class="auth-form mt-[1rem] grid gap-[0.75rem]" onSubmit={submit}>
          <label class="grid gap-[0.34rem] text-[0.9rem] text-ink-1">
            Username
            <input
              class="rounded-[0.66rem] border border-line-soft bg-bg-2 px-[0.78rem] py-[0.74rem] outline-none focus-visible:outline-2 focus-visible:outline-brand focus-visible:outline-offset-[1px]"
              autocomplete="username"
              maxlength="32"
              required
              value={username()}
              onInput={(event) => setUsername(event.currentTarget.value)}
              pattern="[A-Za-z0-9_.]{3,32}"
            />
          </label>

          <label class="grid gap-[0.34rem] text-[0.9rem] text-ink-1">
            Password
            <input
              class="rounded-[0.66rem] border border-line-soft bg-bg-2 px-[0.78rem] py-[0.74rem] outline-none focus-visible:outline-2 focus-visible:outline-brand focus-visible:outline-offset-[1px]"
              type="password"
              autocomplete={isRegisterMode() ? "new-password" : "current-password"}
              minlength="12"
              maxlength="128"
              required
              value={password()}
              onInput={(event) => setPassword(event.currentTarget.value)}
            />
          </label>

          <Show when={shouldRenderCaptcha()}>
            <div class="captcha-block mt-[0.25rem] grid justify-items-start gap-[0.35rem]">
              <div
                class="h-captcha"
                ref={(element) => {
                  captchaContainer = element;
                }}
              />
              <Show when={!captchaReady()}>
                <p class="m-0 text-ink-2">Loading captcha challenge...</p>
              </Show>
              <Show when={captchaError()}>
                <p class="m-0 text-[0.91rem] text-danger" role="alert">
                  {captchaError()}
                </p>
              </Show>
            </div>
          </Show>

          <button
            class="mt-[0.32rem] cursor-pointer rounded-[0.74rem] border-0 bg-gradient-to-b from-brand to-brand-strong px-[0.8rem] py-[0.8rem] font-[750] tracking-[0.015em] text-white disabled:cursor-default disabled:opacity-[0.72]"
            type="submit"
            disabled={isSubmitting()}
          >
            {isSubmitting() ? "Working..." : isRegisterMode() ? "Create account" : "Login"}
          </button>
        </form>

        <Show when={statusMessage()}>
          <p class="mb-0 mt-[0.92rem] text-[0.91rem] text-ok" role="status">
            {statusMessage()}
          </p>
        </Show>
        <Show when={errorMessage()}>
          <p class="mb-0 mt-[0.92rem] text-[0.91rem] text-danger" role="alert">
            {errorMessage()}
          </p>
        </Show>
      </div>
    </div>
  );
}
