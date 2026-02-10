import {
  type Accessor,
  type JSX,
  createContext,
  createEffect,
  createSignal,
  useContext,
} from "solid-js";
import type { AuthSession } from "../domain/auth";
import { clearSession, loadSession, saveSession } from "./session";

interface AuthContextValue {
  session: Accessor<AuthSession | null>;
  setAuthenticatedSession: (session: AuthSession) => void;
  clearAuthenticatedSession: () => void;
}

const AuthContext = createContext<AuthContextValue>();

export function AuthProvider(props: { children: JSX.Element }): JSX.Element {
  const [session, setSession] = createSignal<AuthSession | null>(loadSession());

  createEffect(() => {
    const value = session();
    if (value) {
      saveSession(value);
    } else {
      clearSession();
    }
  });

  const value: AuthContextValue = {
    session,
    setAuthenticatedSession: (nextSession) => setSession(nextSession),
    clearAuthenticatedSession: () => setSession(null),
  };

  return <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const value = useContext(AuthContext);
  if (!value) {
    throw new Error("Auth context not available");
  }
  return value;
}
