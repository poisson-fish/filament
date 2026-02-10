import { fireEvent, render, screen } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

describe("routing", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
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
});
