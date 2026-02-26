import type { JSX } from "solid-js";
import type { MarkdownToken } from "../../../domain/chat";

type ContainerKind = "root" | "p" | "em" | "strong" | "a" | "li" | "ul" | "ol";

interface ContainerNode {
  kind: ContainerKind;
  href?: string;
  children: Array<JSX.Element | string>;
}

export interface SafeMarkdownProps {
  tokens: MarkdownToken[];
  class?: string;
}

export function SafeMarkdown(props: SafeMarkdownProps) {
  const nodes = renderTokens(props.tokens);
  return <div class={props.class}>{nodes}</div>;
}

function renderTokens(tokens: MarkdownToken[]): Array<JSX.Element | string> {
  const stack: ContainerNode[] = [{ kind: "root", children: [] }];

  const append = (value: JSX.Element | string) => {
    const target = stack[stack.length - 1];
    if (!target) {
      return;
    }
    target.children.push(value);
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
        append(containerToElement(node));
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
      append(token.text);
      continue;
    }
    if (token.type === "code") {
      append(<code>{token.code}</code>);
      continue;
    }
    if (token.type === "fenced_code") {
      append(
        <pre>
          <code data-language={token.language ?? undefined}>{token.code}</code>
        </pre>,
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
    append(containerToElement(node));
  }

  return stack[0]?.children ?? [];
}

function containerToElement(node: ContainerNode): JSX.Element | string {
  if (node.kind === "root") {
    return <>{node.children}</>;
  }
  if (node.kind === "p") {
    return <p>{node.children}</p>;
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
    <a href={node.href} target="_blank" rel="noopener noreferrer">
      {node.children}
    </a>
  );
}

function sanitizeLink(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return null;
  }
  try {
    const url = new URL(trimmed);
    if (
      url.protocol === "https:" ||
      url.protocol === "http:" ||
      url.protocol === "mailto:"
    ) {
      return trimmed;
    }
  } catch {
    return null;
  }
  return null;
}
