import { For, Show, createResource, createSignal } from "solid-js";
import { ApiError, fetchMe } from "../lib/api";
import { useAuth } from "../lib/auth-context";

const DEMO_SERVERS = [
  { id: "S", name: "Security" },
  { id: "E", name: "Engineering" },
  { id: "P", name: "Product" },
  { id: "D", name: "Design" },
];

const DEMO_CHANNELS = ["incident-room", "announcements", "backend", "frontend", "random"];

const DEMO_MESSAGES = [
  {
    author: "hardened-bot",
    time: "09:13",
    text: "Daily check: auth/login rate limits healthy across all nodes.",
  },
  {
    author: "ops",
    time: "09:18",
    text: "LiveKit token issuance latency p95 is under 50ms this morning.",
  },
  {
    author: "you",
    time: "09:24",
    text: "Client shell rollout ready. Tracking login UX polish and route guard tests.",
  },
];

function profileErrorMessage(error: unknown): string {
  if (error instanceof ApiError && error.code === "invalid_credentials") {
    return "Session expired. Please login again.";
  }
  return "Profile unavailable.";
}

export function AppShellPage() {
  const auth = useAuth();
  const [composer, setComposer] = createSignal("");
  const [profile] = createResource(async () => {
    const session = auth.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return fetchMe(session);
  });

  const logout = () => {
    auth.clearAuthenticatedSession();
  };

  return (
    <div class="app-shell">
      <aside class="server-rail" aria-label="servers">
        <For each={DEMO_SERVERS}>
          {(server) => <button title={server.name}>{server.id}</button>}
        </For>
      </aside>

      <aside class="channel-rail">
        <header>
          <h2>Filament</h2>
          <span>Secure Workspace</span>
        </header>
        <nav>
          <For each={DEMO_CHANNELS}>{(channel) => <button>#{channel}</button>}</For>
        </nav>
      </aside>

      <main class="chat-panel">
        <header class="chat-header">
          <div>
            <h3>#incident-room</h3>
            <p>Security-first ops channel</p>
          </div>
          <button class="logout" onClick={logout}>
            Logout
          </button>
        </header>

        <section class="message-list" aria-live="polite">
          <For each={DEMO_MESSAGES}>
            {(message) => (
              <article class="message-row">
                <p>
                  <strong>{message.author}</strong>
                  <span>{message.time}</span>
                </p>
                <p>{message.text}</p>
              </article>
            )}
          </For>
        </section>

        <form class="composer" onSubmit={(event) => event.preventDefault()}>
          <input
            value={composer()}
            onInput={(event) => setComposer(event.currentTarget.value)}
            maxlength="2000"
            placeholder="Message #incident-room"
          />
        </form>
      </main>

      <aside class="member-rail">
        <header>
          <h4>Session</h4>
        </header>
        <Show when={profile.loading}>
          <p class="muted">Loading profile...</p>
        </Show>
        <Show when={profile.error}>
          <p class="status error">{profileErrorMessage(profile.error)}</p>
        </Show>
        <Show when={profile()}>
          {(value) => (
            <div class="profile-card">
              <p class="label">Username</p>
              <p>{value().username}</p>
              <p class="label">User ID</p>
              <p class="mono">{value().userId}</p>
            </div>
          )}
        </Show>
      </aside>
    </div>
  );
}
