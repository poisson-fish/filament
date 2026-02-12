import { render } from "solid-js/web";
import { App } from "./App";
import { redirectLoopbackAliasInDev } from "./lib/browser-context";
import "./styles/app.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root container missing");
}

const redirected = redirectLoopbackAliasInDev(
  typeof window !== "undefined" ? window : null,
  import.meta.env.DEV,
);
if (!redirected) {
  render(() => <App />, root);
}
