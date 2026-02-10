import { render, screen } from "@solidjs/testing-library";
import { App } from "../src/App";

describe("routing", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
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
});
