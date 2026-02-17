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

  return (
    <div class="auth-layout">
      <div class="auth-panel">
        <header class="auth-header">
          <p class="eyebrow">Filament</p>
          <h1>{isRegisterMode() ? "Create Account" : "Welcome Back"}</h1>
          <p class="muted">
            {isRegisterMode()
              ? "Register with a valid username and strong password."
              : "Login to enter your workspace."}
          </p>
        </header>

        <div class="auth-mode-switch" role="tablist" aria-label="Authentication mode">
          <button
            type="button"
            classList={{ active: !isRegisterMode() }}
            onClick={() => {
              setRegisterMode(false);
              setCaptchaError("");
            }}
          >
            Login
          </button>
          <button
            type="button"
            classList={{ active: isRegisterMode() }}
            onClick={() => {
              setRegisterMode(true);
              setCaptchaError("");
            }}
          >
            Register
          </button>
        </div>

        <form class="auth-form" onSubmit={submit}>
          <label>
            Username
            <input
              autocomplete="username"
              maxlength="32"
              required
              value={username()}
              onInput={(event) => setUsername(event.currentTarget.value)}
              pattern="[A-Za-z0-9_.]{3,32}"
            />
          </label>

          <label>
            Password
            <input
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
            <div class="captcha-block">
              <div
                class="h-captcha"
                ref={(element) => {
                  captchaContainer = element;
                }}
              />
              <Show when={!captchaReady()}>
                <p class="muted">Loading captcha challenge...</p>
              </Show>
              <Show when={captchaError()}>
                <p class="status error" role="alert">
                  {captchaError()}
                </p>
              </Show>
            </div>
          </Show>

          <button type="submit" disabled={isSubmitting()}>
            {isSubmitting() ? "Working..." : isRegisterMode() ? "Create account" : "Login"}
          </button>
        </form>

        <Show when={statusMessage()}>
          <p class="status ok" role="status">
            {statusMessage()}
          </p>
        </Show>
        <Show when={errorMessage()}>
          <p class="status error" role="alert">
            {errorMessage()}
          </p>
        </Show>
      </div>
    </div>
  );
}
