import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
} from "solid-js";
import { DomainValidationError } from "../domain/auth";
import {
  channelNameFromInput,
  guildNameFromInput,
  messageContentFromInput,
  reactionEmojiFromInput,
  searchQueryFromInput,
  type ChannelId,
  type GuildId,
  type MessageId,
  type MessageRecord,
  type SearchResults,
  type WorkspaceRecord,
} from "../domain/chat";
import {
  ApiError,
  addMessageReaction,
  createChannel,
  createChannelMessage,
  createGuild,
  fetchChannelMessages,
  fetchMe,
  removeMessageReaction,
  searchGuildMessages,
} from "../lib/api";
import { useAuth } from "../lib/auth-context";
import { connectGateway } from "../lib/gateway";
import { clearWorkspaceCache, loadWorkspaceCache, saveWorkspaceCache } from "../lib/workspace-cache";

const THUMBS_UP = reactionEmojiFromInput("👍");

interface ReactionView {
  count: number;
  reacted: boolean;
}

function reactionKey(messageId: MessageId, emoji: string): string {
  return `${messageId}|${emoji}`;
}

function mapError(error: unknown, fallback: string): string {
  if (error instanceof DomainValidationError) {
    return error.message;
  }
  if (error instanceof ApiError) {
    if (error.code === "rate_limited") {
      return "Rate limited. Please wait and retry.";
    }
    if (error.code === "forbidden") {
      return "Permission denied for this action.";
    }
    if (error.code === "not_found") {
      return "Requested resource was not found.";
    }
    if (error.code === "network_error") {
      return "Cannot reach server. Verify API origin and TLS setup.";
    }
    return `Request failed (${error.code}).`;
  }
  return fallback;
}

function profileErrorMessage(error: unknown): string {
  if (error instanceof ApiError && error.code === "invalid_credentials") {
    return "Session expired. Please login again.";
  }
  return mapError(error, "Profile unavailable.");
}

function formatMessageTime(createdAtUnix: number): string {
  return new Date(createdAtUnix * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function shortActor(value: string): string {
  return value.length > 14 ? `${value.slice(0, 14)}...` : value;
}

function upsertWorkspace(
  existing: WorkspaceRecord[],
  guildId: GuildId,
  updater: (workspace: WorkspaceRecord) => WorkspaceRecord,
): WorkspaceRecord[] {
  return existing.map((workspace) => (workspace.guildId === guildId ? updater(workspace) : workspace));
}

function mergeMessage(existing: MessageRecord[], incoming: MessageRecord): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === incoming.messageId);
  if (index >= 0) {
    const next = [...existing];
    next[index] = incoming;
    return next;
  }
  return [...existing, incoming];
}

export function AppShellPage() {
  const auth = useAuth();

  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>(loadWorkspaceCache());
  const [activeGuildId, setActiveGuildId] = createSignal<GuildId | null>(
    workspaces()[0]?.guildId ?? null,
  );
  const [activeChannelId, setActiveChannelId] = createSignal<ChannelId | null>(
    workspaces()[0]?.channels[0]?.channelId ?? null,
  );

  const [composer, setComposer] = createSignal("");
  const [messageStatus, setMessageStatus] = createSignal("");
  const [messageError, setMessageError] = createSignal("");
  const [isLoadingMessages, setLoadingMessages] = createSignal(false);
  const [isSendingMessage, setSendingMessage] = createSignal(false);
  const [messages, setMessages] = createSignal<MessageRecord[]>([]);
  const [reactionState, setReactionState] = createSignal<Record<string, ReactionView>>({});

  const [createGuildName, setCreateGuildName] = createSignal("Security Ops");
  const [createChannelName, setCreateChannelName] = createSignal("incident-room");
  const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
  const [workspaceError, setWorkspaceError] = createSignal("");

  const [newChannelName, setNewChannelName] = createSignal("backend");
  const [isCreatingChannel, setCreatingChannel] = createSignal(false);
  const [channelCreateError, setChannelCreateError] = createSignal("");
  const [showNewChannelForm, setShowNewChannelForm] = createSignal(false);

  const [searchQuery, setSearchQuery] = createSignal("");
  const [searchError, setSearchError] = createSignal("");
  const [isSearching, setSearching] = createSignal(false);
  const [searchResults, setSearchResults] = createSignal<SearchResults | null>(null);

  const [gatewayOnline, setGatewayOnline] = createSignal(false);
  const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);

  const activeWorkspace = createMemo(
    () => workspaces().find((workspace) => workspace.guildId === activeGuildId()) ?? null,
  );

  const activeChannel = createMemo(
    () =>
      activeWorkspace()?.channels.find((channel) => channel.channelId === activeChannelId()) ??
      null,
  );

  const [profile] = createResource(async () => {
    const session = auth.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return fetchMe(session);
  });

  createEffect(() => {
    saveWorkspaceCache(workspaces());
  });

  createEffect(() => {
    const selectedGuild = activeGuildId();
    if (!selectedGuild || !workspaces().some((workspace) => workspace.guildId === selectedGuild)) {
      setActiveGuildId(workspaces()[0]?.guildId ?? null);
      return;
    }

    const channel = activeChannelId();
    const workspace = workspaces().find((entry) => entry.guildId === selectedGuild);
    if (!workspace) {
      return;
    }
    if (!channel || !workspace.channels.some((entry) => entry.channelId === channel)) {
      setActiveChannelId(workspace.channels[0]?.channelId ?? null);
    }
  });

  const refreshMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessages([]);
      return;
    }

    setMessageError("");
    setLoadingMessages(true);
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, { limit: 50 });
      setMessages([...history.messages].reverse());
    } catch (error) {
      setMessageError(mapError(error, "Unable to load messages."));
      setMessages([]);
    } finally {
      setLoadingMessages(false);
    }
  };

  createEffect(() => {
    void activeGuildId();
    void activeChannelId();
    setReactionState({});
    void refreshMessages();
  });

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setGatewayOnline(false);
      setOnlineMembers([]);
      return;
    }

    const gateway = connectGateway(session.accessToken, guildId, channelId, {
      onOpenStateChange: (isOpen) => setGatewayOnline(isOpen),
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        setMessages((existing) => mergeMessage(existing, message));
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers((existing) => {
          if (payload.status === "online") {
            return existing.includes(payload.userId) ? existing : [...existing, payload.userId];
          }
          return existing.filter((entry) => entry !== payload.userId);
        });
      },
    });

    onCleanup(() => gateway.close());
  });

  const createFirstWorkspace = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session) {
      setWorkspaceError("Missing auth session.");
      return;
    }
    if (isCreatingWorkspace()) {
      return;
    }

    setWorkspaceError("");
    setCreatingWorkspace(true);
    try {
      const guild = await createGuild(session, { name: guildNameFromInput(createGuildName()) });
      const channel = await createChannel(session, guild.guildId, {
        name: channelNameFromInput(createChannelName()),
      });
      const createdWorkspace: WorkspaceRecord = {
        guildId: guild.guildId,
        guildName: guild.name,
        channels: [channel],
      };
      setWorkspaces((existing) => [...existing, createdWorkspace]);
      setActiveGuildId(createdWorkspace.guildId);
      setActiveChannelId(channel.channelId);
      setMessageStatus("Workspace created.");
    } catch (error) {
      setWorkspaceError(mapError(error, "Unable to create workspace."));
    } finally {
      setCreatingWorkspace(false);
    }
  };

  const createNewChannel = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setChannelCreateError("Select a workspace first.");
      return;
    }
    if (isCreatingChannel()) {
      return;
    }

    setChannelCreateError("");
    setCreatingChannel(true);
    try {
      const created = await createChannel(session, guildId, {
        name: channelNameFromInput(newChannelName()),
      });
      setWorkspaces((existing) =>
        upsertWorkspace(existing, guildId, (workspace) => {
          if (workspace.channels.some((channel) => channel.channelId === created.channelId)) {
            return workspace;
          }
          return {
            ...workspace,
            channels: [...workspace.channels, created],
          };
        }),
      );
      setActiveChannelId(created.channelId);
      setShowNewChannelForm(false);
      setNewChannelName("backend");
    } catch (error) {
      setChannelCreateError(mapError(error, "Unable to create channel."));
    } finally {
      setCreatingChannel(false);
    }
  };

  const sendMessage = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessageError("Select a channel first.");
      return;
    }

    if (isSendingMessage()) {
      return;
    }

    setMessageError("");
    setMessageStatus("");
    setSendingMessage(true);
    try {
      const created = await createChannelMessage(session, guildId, channelId, {
        content: messageContentFromInput(composer()),
      });
      setMessages((existing) => mergeMessage(existing, created));
      setComposer("");
    } catch (error) {
      setMessageError(mapError(error, "Unable to send message."));
    } finally {
      setSendingMessage(false);
    }
  };

  const toggleThumbsUp = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, THUMBS_UP);
    const state = reactionState()[key] ?? { count: 0, reacted: false };

    try {
      if (state.reacted) {
        const response = await removeMessageReaction(session, guildId, channelId, messageId, THUMBS_UP);
        setReactionState((existing) => ({
          ...existing,
          [key]: { count: response.count, reacted: false },
        }));
      } else {
        const response = await addMessageReaction(session, guildId, channelId, messageId, THUMBS_UP);
        setReactionState((existing) => ({
          ...existing,
          [key]: { count: response.count, reacted: true },
        }));
      }
    } catch (error) {
      setMessageError(mapError(error, "Unable to update reaction."));
    }
  };

  const runSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setSearchError("Select a workspace first.");
      return;
    }

    if (isSearching()) {
      return;
    }

    setSearching(true);
    setSearchError("");
    try {
      const results = await searchGuildMessages(session, guildId, {
        query: searchQueryFromInput(searchQuery()),
        limit: 12,
        channelId: activeChannelId() ?? undefined,
      });
      setSearchResults(results);
    } catch (error) {
      setSearchError(mapError(error, "Search request failed."));
      setSearchResults(null);
    } finally {
      setSearching(false);
    }
  };

  const logout = () => {
    auth.clearAuthenticatedSession();
    clearWorkspaceCache();
  };

  return (
    <div class="app-shell">
      <aside class="server-rail" aria-label="servers">
        <header class="rail-label">WS</header>
        <For each={workspaces()}>
          {(workspace) => (
            <button
              title={workspace.guildName}
              classList={{ active: activeGuildId() === workspace.guildId }}
              onClick={() => {
                setActiveGuildId(workspace.guildId);
                setActiveChannelId(workspace.channels[0]?.channelId ?? null);
              }}
            >
              {workspace.guildName.slice(0, 1).toUpperCase()}
            </button>
          )}
        </For>
      </aside>

      <aside class="channel-rail">
        <header>
          <h2>{activeWorkspace()?.guildName ?? "No Workspace"}</h2>
          <span>Hardened workspace</span>
        </header>

        <Switch>
          <Match when={!activeWorkspace()}>
            <p class="muted">Create a workspace to begin.</p>
          </Match>
          <Match when={activeWorkspace()}>
            <nav aria-label="channels">
              <p class="group-label">TEXT CHANNELS</p>
              <For each={activeWorkspace()?.channels ?? []}>
                {(channel) => (
                  <button
                    classList={{ active: activeChannelId() === channel.channelId }}
                    onClick={() => setActiveChannelId(channel.channelId)}
                  >
                    <span>#{channel.name}</span>
                  </button>
                )}
              </For>

              <button class="create-channel-toggle" onClick={() => setShowNewChannelForm((v) => !v)}>
                {showNewChannelForm() ? "Cancel" : "New channel"}
              </button>

              <Show when={showNewChannelForm()}>
                <form class="inline-form" onSubmit={createNewChannel}>
                  <label>
                    Channel name
                    <input
                      value={newChannelName()}
                      onInput={(event) => setNewChannelName(event.currentTarget.value)}
                      maxlength="64"
                    />
                  </label>
                  <button type="submit" disabled={isCreatingChannel()}>
                    {isCreatingChannel() ? "Creating..." : "Create"}
                  </button>
                </form>
                <Show when={channelCreateError()}>
                  <p class="status error">{channelCreateError()}</p>
                </Show>
              </Show>
            </nav>
          </Match>
        </Switch>
      </aside>

      <main class="chat-panel">
        <header class="chat-header">
          <div>
            <h3>{activeChannel() ? `#${activeChannel()!.name}` : "#no-channel"}</h3>
            <p>Gateway {gatewayOnline() ? "connected" : "disconnected"}</p>
          </div>
          <div class="header-actions">
            <span classList={{ "gateway-badge": true, online: gatewayOnline() }}>
              {gatewayOnline() ? "Live" : "Offline"}
            </span>
            <button type="button" onClick={() => void refreshMessages()}>
              Refresh
            </button>
            <button class="logout" onClick={logout}>
              Logout
            </button>
          </div>
        </header>

        <Show when={workspaces().length === 0} fallback={
          <>
            <Show when={isLoadingMessages()}>
              <p class="panel-note">Loading messages...</p>
            </Show>
            <Show when={messageError()}>
              <p class="status error panel-note">{messageError()}</p>
            </Show>
            <section class="message-list" aria-live="polite">
              <For each={messages()}>
                {(message) => {
                  const state = () => reactionState()[reactionKey(message.messageId, THUMBS_UP)] ?? { count: 0, reacted: false };
                  return (
                    <article class="message-row">
                      <p>
                        <strong>{shortActor(message.authorId)}</strong>
                        <span>{formatMessageTime(message.createdAtUnix)}</span>
                      </p>
                      <p>{message.content}</p>
                      <div class="reaction-row">
                        <button
                          type="button"
                          classList={{ reacted: state().reacted }}
                          onClick={() => void toggleThumbsUp(message.messageId)}
                        >
                          {THUMBS_UP} {state().count}
                        </button>
                      </div>
                    </article>
                  );
                }}
              </For>
              <Show when={!isLoadingMessages() && messages().length === 0 && !messageError()}>
                <p class="muted">No messages yet in this channel.</p>
              </Show>
            </section>
            <form class="composer" onSubmit={sendMessage}>
              <input
                value={composer()}
                onInput={(event) => setComposer(event.currentTarget.value)}
                maxlength="2000"
                placeholder={activeChannel() ? `Message #${activeChannel()!.name}` : "Select channel"}
                disabled={!activeChannel() || isSendingMessage()}
              />
              <button type="submit" disabled={!activeChannel() || isSendingMessage()}>
                {isSendingMessage() ? "Sending..." : "Send"}
              </button>
            </form>
          </>
        }>
          <section class="empty-workspace">
            <h3>Create your first workspace</h3>
            <p class="muted">The API currently exposes create routes, so this client provisions your first guild/channel here.</p>
            <form class="inline-form" onSubmit={createFirstWorkspace}>
              <label>
                Workspace name
                <input
                  value={createGuildName()}
                  onInput={(event) => setCreateGuildName(event.currentTarget.value)}
                  maxlength="64"
                />
              </label>
              <label>
                First channel
                <input
                  value={createChannelName()}
                  onInput={(event) => setCreateChannelName(event.currentTarget.value)}
                  maxlength="64"
                />
              </label>
              <button type="submit" disabled={isCreatingWorkspace()}>
                {isCreatingWorkspace() ? "Creating..." : "Create workspace"}
              </button>
            </form>
            <Show when={workspaceError()}>
              <p class="status error">{workspaceError()}</p>
            </Show>
          </section>
        </Show>

        <Show when={messageStatus()}>
          <p class="status ok panel-note">{messageStatus()}</p>
        </Show>
      </main>

      <aside class="member-rail">
        <header>
          <h4>Profile + Search</h4>
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

        <section class="member-group">
          <p class="group-label">ONLINE ({onlineMembers().length})</p>
          <ul>
            <For each={onlineMembers()}>
              {(memberId) => (
                <li>
                  <span class="presence online" />
                  {shortActor(memberId)}
                </li>
              )}
            </For>
            <Show when={onlineMembers().length === 0}>
              <li>
                <span class="presence idle" />
                no-presence-yet
              </li>
            </Show>
          </ul>
        </section>

        <section class="member-group">
          <p class="group-label">SEARCH</p>
          <form class="inline-form" onSubmit={runSearch}>
            <label>
              Query
              <input
                value={searchQuery()}
                onInput={(event) => setSearchQuery(event.currentTarget.value)}
                maxlength="256"
                placeholder="needle"
              />
            </label>
            <button type="submit" disabled={isSearching() || !activeWorkspace()}>
              {isSearching() ? "Searching..." : "Search"}
            </button>
          </form>
          <Show when={searchError()}>
            <p class="status error">{searchError()}</p>
          </Show>
          <Show when={searchResults()}>
            {(results) => (
              <ul>
                <For each={results().messages}>
                  {(message) => (
                    <li>
                      <span class="presence online" />
                      {shortActor(message.authorId)}: {message.content.slice(0, 32)}
                    </li>
                  )}
                </For>
              </ul>
            )}
          </Show>
        </section>
      </aside>
    </div>
  );
}
