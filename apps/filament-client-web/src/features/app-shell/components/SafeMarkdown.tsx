import { Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import type { MarkdownToken } from "../../../domain/chat";
import {
  createFilamentMarkdownHighlighter,
  resolveHighlightLanguage,
} from "./markdown-highlight";
import { renderEmojiMixedText } from "./messages/emoji-utils";

type HeadingContainerKind = "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
type ContainerKind =
  | "root"
  | "p"
  | HeadingContainerKind
  | "em"
  | "strong"
  | "a"
  | "li"
  | "ul"
  | "ol";

interface ContainerNode {
  kind: ContainerKind;
  href?: string;
  children: Array<JSX.Element | string>;
}

export interface SafeMarkdownProps {
  tokens: MarkdownToken[];
  class?: string;
}

interface ExternalLinkConfirmState {
  url: string;
  host: string | null;
}

const markdownHighlighter = createFilamentMarkdownHighlighter();

export function SafeMarkdown(props: SafeMarkdownProps) {
  const [pendingExternalLink, setPendingExternalLink] =
    createSignal<ExternalLinkConfirmState | null>(null);
  const nodes = renderTokens(props.tokens, (event, href) => {
    event.preventDefault();
    const parsedHref = normalizeExternalUrl(href);
    if (!parsedHref) {
      return;
    }
    setPendingExternalLink({
      url: parsedHref.toString(),
      host: parsedHref.host || null,
    });
  });

  return (
    <>
      <div class={`safe-markdown ${props.class ?? ""}`.trim()}>{nodes}</div>
      <Show when={pendingExternalLink()}>
        {(stateAccessor) => {
          const state = stateAccessor();
          return (
            <div class="fixed inset-0 z-40 grid place-items-center bg-black/70 p-[0.9rem]">
              <section
                role="dialog"
                aria-modal="true"
                aria-label="External link confirmation"
                class="external-link-confirm-modal w-full max-w-[34rem] rounded-[0.9rem] border border-line bg-bg-1 p-[1rem] shadow-panel"
              >
                <div class="grid gap-[0.45rem]">
                  <h3 class="m-0 text-[1.36rem] font-[760] text-ink-0">Leaving Filament</h3>
                  <p class="m-0 text-[1.05rem] text-ink-2">
                    This link is taking you to the following website:
                  </p>
                </div>
                <pre class="m-[1rem_0_0_0] overflow-auto rounded-[0.78rem] border border-line-soft bg-bg-2 p-[0.78rem] text-[1.02rem] text-ink-1 whitespace-pre-wrap break-all">
                  {state.url}
                </pre>
                <Show when={state.host}>
                  <p class="m-[0.65rem_0_0_0] text-[0.95rem] text-ink-2">
                    Destination host: {state.host}
                  </p>
                </Show>
                <div class="mt-[1rem] grid grid-cols-2 gap-[0.65rem] max-[620px]:grid-cols-1">
                  <button
                    type="button"
                    class="rounded-[0.72rem] border border-line-soft bg-bg-3 px-[0.92rem] py-[0.7rem] text-[1.02rem] font-[640] text-ink-0 transition-colors hover:bg-bg-4"
                    onClick={() => setPendingExternalLink(null)}
                  >
                    Go Back
                  </button>
                  <button
                    type="button"
                    class="rounded-[0.72rem] border border-brand bg-brand px-[0.92rem] py-[0.7rem] text-[1.02rem] font-[700] text-white transition-colors hover:bg-brand-strong"
                    onClick={() => {
                      openExternalLink(state.url);
                      setPendingExternalLink(null);
                    }}
                  >
                    Visit Site
                  </button>
                </div>
              </section>
            </div>
          );
        }}
      </Show>
    </>
  );
}

function renderTokens(
  tokens: MarkdownToken[],
  onOpenExternalLink: (event: MouseEvent, href: string) => void,
): Array<JSX.Element | string> {
  const stack: ContainerNode[] = [{ kind: "root", children: [] }];

  const append = (value: JSX.Element | string) => {
    const target = stack[stack.length - 1];
    if (!target) {
      return;
    }
    target.children.push(value);
  };
  const appendText = (text: string) => {
    for (const chunk of renderEmojiMixedText(text)) {
      append(chunk);
    }
  };

  const push = (kind: ContainerKind, href?: string) => {
    stack.push({ kind, href, children: [] });
  };

  const closeOne = (kind: ContainerKind) => {
    for (let index = stack.length - 1; index > 0; index -= 1) {
      if (stack[index]?.kind !== kind) {
        continue;
      }
      while (stack.length - 1 >= index) {
        const node = stack.pop();
        if (!node) {
          break;
        }
        append(containerToElement(node, onOpenExternalLink));
      }
      return;
    }
  };

  for (const token of tokens) {
    if (token.type === "paragraph_start") {
      push("p");
      continue;
    }
    if (token.type === "paragraph_end") {
      closeOne("p");
      continue;
    }
    if (token.type === "heading_start") {
      push(headingKindFromLevel(token.level));
      continue;
    }
    if (token.type === "heading_end") {
      closeHeadingContainer(closeOne);
      continue;
    }
    if (token.type === "list_start") {
      push(token.ordered ? "ol" : "ul");
      continue;
    }
    if (token.type === "list_end") {
      closeOne("ul");
      closeOne("ol");
      continue;
    }
    if (token.type === "list_item_start") {
      push("li");
      continue;
    }
    if (token.type === "list_item_end") {
      closeOne("li");
      continue;
    }
    if (token.type === "emphasis_start") {
      push("em");
      continue;
    }
    if (token.type === "emphasis_end") {
      closeOne("em");
      continue;
    }
    if (token.type === "strong_start") {
      push("strong");
      continue;
    }
    if (token.type === "strong_end") {
      closeOne("strong");
      continue;
    }
    if (token.type === "link_start") {
      const href = sanitizeLink(token.href);
      if (href) {
        push("a", href);
      }
      continue;
    }
    if (token.type === "link_end") {
      closeOne("a");
      continue;
    }
    if (token.type === "text") {
      appendText(token.text);
      continue;
    }
    if (token.type === "code") {
      append(<code>{renderEmojiMixedText(token.code)}</code>);
      continue;
    }
    if (token.type === "fenced_code") {
      const language = resolveHighlightLanguage(token.language);
      const codeChildren = language
        ? renderHighlightedCode(token.code, language)
        : [token.code];
      append(
        <div class="safe-markdown-code-block">
          <ShowLanguageLabel language={language} />
          <pre>
            <code data-language={language ?? undefined}>{codeChildren}</code>
          </pre>
        </div>,
      );
      continue;
    }
    if (token.type === "soft_break" || token.type === "hard_break") {
      append(<br />);
    }
  }

  while (stack.length > 1) {
    const node = stack.pop();
    if (!node) {
      break;
    }
    append(containerToElement(node, onOpenExternalLink));
  }

  return stack[0]?.children ?? [];
}

function ShowLanguageLabel(props: { language: string | null }): JSX.Element {
  if (!props.language) {
    return <p class="safe-markdown-code-label">code</p>;
  }
  return <p class="safe-markdown-code-label">{props.language}</p>;
}

function renderHighlightedCode(code: string, language: string): Array<JSX.Element | string> {
  const tree = markdownHighlighter.highlight(language, code);
  return tree.children.flatMap((child, index) => renderHighlightNode(child, `${index}`));
}

function renderHighlightNode(
  node: unknown,
  key: string,
): Array<JSX.Element | string> {
  if (!node || typeof node !== "object") {
    return [];
  }
  const textNode = node as { type?: unknown; value?: unknown };
  if (textNode.type === "text" && typeof textNode.value === "string") {
    return renderEmojiMixedText(textNode.value);
  }

  const elementNode = node as {
    type?: unknown;
    tagName?: unknown;
    properties?: unknown;
    children?: unknown;
  };
  if (elementNode.type !== "element" || elementNode.tagName !== "span") {
    if (Array.isArray(elementNode.children)) {
      return elementNode.children.flatMap((child, childIndex) =>
        renderHighlightNode(child, `${key}-${childIndex}`),
      );
    }
    return [];
  }

  const classNames = extractHighlightClassNames(elementNode.properties);
  const children = Array.isArray(elementNode.children)
    ? elementNode.children.flatMap((child, childIndex) =>
        renderHighlightNode(child, `${key}-${childIndex}`),
      )
    : [];

  return [
    <span class={classNames.length > 0 ? classNames.join(" ") : undefined}>
      {children}
    </span>,
  ];
}

function extractHighlightClassNames(properties: unknown): string[] {
  if (!properties || typeof properties !== "object") {
    return [];
  }
  const record = properties as { className?: unknown };
  if (!Array.isArray(record.className)) {
    return [];
  }
  return record.className.filter(
    (entry): entry is string =>
      typeof entry === "string" && /^hljs[0-9a-z-]*$/i.test(entry),
  );
}

function containerToElement(
  node: ContainerNode,
  onOpenExternalLink: (event: MouseEvent, href: string) => void,
): JSX.Element | string {
  if (node.kind === "root") {
    return <>{node.children}</>;
  }
  if (node.kind === "p") {
    return <p>{node.children}</p>;
  }
  if (node.kind === "h1") {
    return <h1>{node.children}</h1>;
  }
  if (node.kind === "h2") {
    return <h2>{node.children}</h2>;
  }
  if (node.kind === "h3") {
    return <h3>{node.children}</h3>;
  }
  if (node.kind === "h4") {
    return <h4>{node.children}</h4>;
  }
  if (node.kind === "h5") {
    return <h5>{node.children}</h5>;
  }
  if (node.kind === "h6") {
    return <h6>{node.children}</h6>;
  }
  if (node.kind === "em") {
    return <em>{node.children}</em>;
  }
  if (node.kind === "strong") {
    return <strong>{node.children}</strong>;
  }
  if (node.kind === "li") {
    return <li>{node.children}</li>;
  }
  if (node.kind === "ul") {
    return <ul>{node.children}</ul>;
  }
  if (node.kind === "ol") {
    return <ol>{node.children}</ol>;
  }
  if (!node.href) {
    return <>{node.children}</>;
  }
  return (
    <a
      href={node.href}
      target="_blank"
      rel="noopener noreferrer"
      onAuxClick={(event) => onOpenExternalLink(event, node.href!)}
      onContextMenu={(event) => event.preventDefault()}
      onDragStart={(event) => event.preventDefault()}
      onClick={(event) => onOpenExternalLink(event, node.href!)}
    >
      {node.children}
    </a>
  );
}

function sanitizeLink(raw: string): string | null {
  return normalizeExternalUrl(raw)?.toString() ?? null;
}

function headingKindFromLevel(level: number): HeadingContainerKind {
  if (level === 1) {
    return "h1";
  }
  if (level === 2) {
    return "h2";
  }
  if (level === 3) {
    return "h3";
  }
  if (level === 4) {
    return "h4";
  }
  if (level === 5) {
    return "h5";
  }
  return "h6";
}

function closeHeadingContainer(closeOne: (kind: ContainerKind) => void): void {
  closeOne("h6");
  closeOne("h5");
  closeOne("h4");
  closeOne("h3");
  closeOne("h2");
  closeOne("h1");
}

function openExternalLink(url: string): void {
  const sanitized = sanitizeLink(url);
  if (!sanitized) {
    return;
  }
  window.open(sanitized, "_blank", "noopener,noreferrer");
}

function normalizeExternalUrl(rawUrl: string): URL | null {
  const trimmed = rawUrl.trim();
  if (trimmed.length === 0) {
    return null;
  }

  try {
    const parsed = new URL(trimmed);
    if (
      parsed.protocol !== "https:" &&
      parsed.protocol !== "http:" &&
      parsed.protocol !== "mailto:"
    ) {
      return null;
    }
    if (parsed.username.length > 0 || parsed.password.length > 0) {
      return null;
    }
    if ((parsed.protocol === "https:" || parsed.protocol === "http:") && !parsed.hostname) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}
