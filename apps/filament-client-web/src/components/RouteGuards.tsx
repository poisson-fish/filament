import { Show, type JSX } from "solid-js";
import { Navigate } from "@solidjs/router";
import { useAuth } from "../lib/auth-context";
import { isSessionExpired } from "../lib/session";

export function RequireAuth(props: { children: JSX.Element }): JSX.Element {
  const auth = useAuth();
  const isAuthed = () => {
    const session = auth.session();
    return Boolean(session && !isSessionExpired(session));
  };
  return (
    <Show when={isAuthed()} fallback={<Navigate href="/login" />}>
      {props.children}
    </Show>
  );
}

export function RedirectAuthedToApp(): JSX.Element {
  const auth = useAuth();
  const isAuthed = () => {
    const session = auth.session();
    return Boolean(session && !isSessionExpired(session));
  };
  return (
    <Show when={isAuthed()} fallback={<Navigate href="/login" />}>
      <Navigate href="/app" />
    </Show>
  );
}
