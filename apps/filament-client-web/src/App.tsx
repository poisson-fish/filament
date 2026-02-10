import { Navigate, Route, Router } from "@solidjs/router";
import { RequireAuth, RedirectAuthedToApp } from "./components/RouteGuards";
import { AuthProvider } from "./lib/auth-context";
import { AppShellPage } from "./pages/AppShellPage";
import { LoginPage } from "./pages/LoginPage";

export function App() {
  return (
    <AuthProvider>
      <Router>
        <Route path="/" component={RedirectAuthedToApp} />
        <Route path="/login" component={LoginPage} />
        <Route
          path="/app"
          component={() => (
            <RequireAuth>
              <AppShellPage />
            </RequireAuth>
          )}
        />
        <Route path="*" component={() => <Navigate href="/" />} />
      </Router>
    </AuthProvider>
  );
}
