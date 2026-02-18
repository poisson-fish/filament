import { render } from "solid-js/web";
import { App } from "./App";
import { redirectLoopbackAliasInDev } from "./lib/browser-context";
import { installViewportHeightCssVar } from "./lib/viewport-height";
import "uno.css";
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
  const teardownViewportHeightSync = installViewportHeightCssVar(
    typeof window !== "undefined" ? window : null,
  );
  if (import.meta.hot) {
    import.meta.hot.dispose(() => {
      teardownViewportHeightSync();
    });
  }
  render(() => <App />, root);
}
