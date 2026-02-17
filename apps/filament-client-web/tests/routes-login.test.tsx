import { fireEvent, render, screen } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

describe("routing", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("renders login page at /login", async () => {
    window.history.replaceState({}, "", "/login");
    render(() => <App />);
    expect(await screen.findByRole("heading", { name: "Welcome Back" })).toBeInTheDocument();
  });

  it("redirects unauthenticated root to /login", async () => {
    window.history.replaceState({}, "", "/");
    render(() => <App />);
    expect(await screen.findByRole("heading", { name: "Welcome Back" })).toBeInTheDocument();
  });

  it("navigates to app shell after successful login submit", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/auth/login")) {
        return new Response(
          JSON.stringify({
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_in_secs: 3600,
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.includes("/auth/me")) {
        return new Response(
          JSON.stringify({
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            username: "alice",
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.includes("/guilds/public")) {
        return new Response(JSON.stringify({ guilds: [] }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(JSON.stringify({ error: "not_found" }), {
        status: 404,
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    window.history.replaceState({}, "", "/login");
    render(() => <App />);

    await fireEvent.input(screen.getByLabelText("Username"), { target: { value: "alice" } });
    await fireEvent.input(screen.getByLabelText("Password"), { target: { value: "supersecure12" } });
    const submitButton = document.querySelector(
      ".auth-form button[type='submit']",
    ) as HTMLButtonElement | null;
    expect(submitButton).not.toBeNull();
    await fireEvent.click(submitButton!);

    expect(await screen.findByRole("heading", { name: "Create your first workspace" })).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalled();
  });

  it("sends captcha token during registration when hcaptcha is configured", async () => {
    vi.stubEnv("VITE_FILAMENT_HCAPTCHA_SITE_KEY", "10000000-ffff-ffff-ffff-000000000001");
    vi.stubGlobal("hcaptcha", {
      render: (_container: HTMLElement, config: { callback: (token: string) => void }) => {
        config.callback("tok_222222222222222222222222222222222222");
        return "widget_1";
      },
      reset: () => {},
      remove: () => {},
    });
    vi.spyOn(document.head, "appendChild").mockImplementation((node) => {
      const script = node as HTMLScriptElement;
      if (typeof script.onload === "function") {
        script.onload(new Event("load"));
      }
      return node;
    });

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes("/auth/register")) {
        const rawBody = typeof init?.body === "string" ? init.body : "";
        const body = JSON.parse(rawBody) as { captcha_token?: string };
        expect(body.captcha_token).toBe("tok_222222222222222222222222222222222222");
        return new Response(JSON.stringify({ accepted: true }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(JSON.stringify({ error: "not_found" }), {
        status: 404,
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    window.history.replaceState({}, "", "/login");
    render(() => <App />);

    await fireEvent.click(screen.getByRole("button", { name: "Register" }));
    await fireEvent.input(screen.getByLabelText("Username"), { target: { value: "alice_77" } });
    await fireEvent.input(screen.getByLabelText("Password"), { target: { value: "supersecure12" } });
    const submitButton = document.querySelector(
      ".auth-form button[type='submit']",
    ) as HTMLButtonElement | null;
    expect(submitButton).not.toBeNull();
    await fireEvent.click(submitButton!);

    expect(await screen.findByText("Account accepted. Continue with login.")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("renders captcha and retries login with captcha token when backend requires it", async () => {
    vi.stubEnv("VITE_FILAMENT_HCAPTCHA_SITE_KEY", "10000000-ffff-ffff-ffff-000000000001");
    vi.stubGlobal("hcaptcha", {
      render: (_container: HTMLElement, config: { callback: (token: string) => void }) => {
        config.callback("tok_login_22222222222222222222222222222222");
        return "widget_login_1";
      },
      reset: () => {},
      remove: () => {},
    });
    vi.spyOn(document.head, "appendChild").mockImplementation((node) => {
      const script = node as HTMLScriptElement;
      if (typeof script.onload === "function") {
        script.onload(new Event("load"));
      }
      return node;
    });

    let loginAttemptCount = 0;
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes("/auth/login")) {
        loginAttemptCount += 1;
        const rawBody = typeof init?.body === "string" ? init.body : "";
        const body = JSON.parse(rawBody) as { captcha_token?: string };
        if (loginAttemptCount === 1) {
          expect(body.captcha_token).toBeUndefined();
          return new Response(JSON.stringify({ error: "captcha_failed" }), {
            status: 403,
            headers: { "content-type": "application/json" },
          });
        }
        expect(body.captcha_token).toBe("tok_login_22222222222222222222222222222222");
        return new Response(
          JSON.stringify({
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_in_secs: 3600,
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.includes("/auth/me")) {
        return new Response(
          JSON.stringify({
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            username: "alice",
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.includes("/guilds/public")) {
        return new Response(JSON.stringify({ guilds: [] }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(JSON.stringify({ error: "not_found" }), {
        status: 404,
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    window.history.replaceState({}, "", "/login");
    render(() => <App />);

    await fireEvent.input(screen.getByLabelText("Username"), { target: { value: "alice" } });
    await fireEvent.input(screen.getByLabelText("Password"), { target: { value: "supersecure12" } });
    const submitButton = document.querySelector(
      ".auth-form button[type='submit']",
    ) as HTMLButtonElement | null;
    expect(submitButton).not.toBeNull();

    await fireEvent.click(submitButton!);
    expect(await screen.findByText("Captcha verification failed. Please retry.")).toBeInTheDocument();
    expect(document.querySelector(".captcha-block .h-captcha")).not.toBeNull();

    await fireEvent.click(submitButton!);

    expect(await screen.findByRole("heading", { name: "Create your first workspace" })).toBeInTheDocument();
    expect(loginAttemptCount).toBe(2);
  });
});
