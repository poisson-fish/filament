import type { JSX } from "solid-js";
import { Navigate } from "@solidjs/router";
import { useAuth } from "../lib/auth-context";
import { isSessionExpired } from "../lib/session";

export function RequireAuth(props: { children: JSX.Element }): JSX.Element {
  const auth = useAuth();
  const session = auth.session();

  if (!session || isSessionExpired(session)) {
    return <Navigate href="/login" />;
  }

  return props.children;
}

export function RedirectAuthedToApp(): JSX.Element {
  const auth = useAuth();
  const session = auth.session();
  if (session && !isSessionExpired(session)) {
    return <Navigate href="/app" />;
  }
  return <Navigate href="/login" />;
}
