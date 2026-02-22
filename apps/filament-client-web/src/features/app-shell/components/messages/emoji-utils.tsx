import twitterData from "@emoji-mart/data/sets/14/twitter.json";
import "emoji-mart";
import { init } from "emoji-mart";
import type { JSX } from "solid-js";

declare module "solid-js" {
    namespace JSX {
        interface IntrinsicElements {
            "em-emoji": any;
        }
    }
}

let initialized = false;
let shortcodeMap: Record<string, string> | null = null;

export function initEmojiMart() {
  if (!initialized) {
    init({ data: twitterData, set: "native", theme: "auto" });
    initialized = true;
  }
}

interface EmojiSkinRecord {
  native?: string;
}

interface EmojiRecord {
  id?: string;
  aliases?: string[];
  shortcodes?: string[];
  emoticons?: string[];
  skins?: EmojiSkinRecord[];
}

interface EmojiDataRecord {
  emojis?: Record<string, EmojiRecord>;
}

function normalizeShortcodeToken(token: string): string {
  return token.toLowerCase();
}

function maybeMapShortcode(map: Record<string, string>, shortcode: string, native: string): void {
  if (/^:[a-zA-Z0-9_+\-]+:$/.test(shortcode)) {
    map[normalizeShortcodeToken(shortcode)] = native;
  }
}

export function buildShortcodeMap() {
  if (shortcodeMap) {
    return shortcodeMap;
  }

  shortcodeMap = {};
  const data = twitterData as EmojiDataRecord;
  const emojis = data.emojis ?? {};
  for (const emoji of Object.values(emojis)) {
    const native = emoji.skins?.[0]?.native;
    if (typeof native !== "string" || native.length === 0) {
      continue;
    }

    if (typeof emoji.id === "string" && emoji.id.length > 0) {
      maybeMapShortcode(shortcodeMap, `:${emoji.id}:`, native);
    }

    for (const alias of emoji.aliases ?? []) {
      maybeMapShortcode(shortcodeMap, `:${alias}:`, native);
    }
    for (const shortcode of emoji.shortcodes ?? []) {
      maybeMapShortcode(shortcodeMap, shortcode, native);
    }
    for (const emoticon of emoji.emoticons ?? []) {
      maybeMapShortcode(shortcodeMap, emoticon, native);
    }
  }

  return shortcodeMap;
}

const SHORTCODE_PATTERN = /:([a-zA-Z0-9_+\-]+):/g;

function remapSelectionPosition(
  position: number | null,
  matchStart: number,
  matchEnd: number,
  replacementLength: number,
): number | null {
  if (position === null) {
    return null;
  }
  if (position <= matchStart) {
    return position;
  }
  if (position >= matchEnd) {
    return position + (replacementLength - (matchEnd - matchStart));
  }

  return matchStart + replacementLength;
}

export interface ShortcodeReplacementResult {
  text: string;
  selectionStart: number | null;
  selectionEnd: number | null;
}

export function replaceEmojiShortcodesWithSelection(
  text: string,
  selectionStart: number | null,
  selectionEnd: number | null,
): ShortcodeReplacementResult {
  if (!text.includes(":")) {
    return { text, selectionStart, selectionEnd };
  }

  const map = buildShortcodeMap();
  let nextText = "";
  let changed = false;
  let previousEnd = 0;
  let nextSelectionStart = selectionStart;
  let nextSelectionEnd = selectionEnd;

  SHORTCODE_PATTERN.lastIndex = 0;
  let match = SHORTCODE_PATTERN.exec(text);
  while (match) {
    const shortcode = match[0];
    const replacement = map[normalizeShortcodeToken(shortcode)];
    const matchStart = match.index;
    const matchEnd = match.index + shortcode.length;
    nextText += text.slice(previousEnd, matchStart);

    if (typeof replacement === "string") {
      changed = true;
      nextText += replacement;
      nextSelectionStart = remapSelectionPosition(
        nextSelectionStart,
        matchStart,
        matchEnd,
        replacement.length,
      );
      nextSelectionEnd = remapSelectionPosition(
        nextSelectionEnd,
        matchStart,
        matchEnd,
        replacement.length,
      );
    } else {
      nextText += shortcode;
    }

    previousEnd = matchEnd;
    match = SHORTCODE_PATTERN.exec(text);
  }

  if (!changed) {
    return { text, selectionStart, selectionEnd };
  }

  nextText += text.slice(previousEnd);
  return {
    text: nextText,
    selectionStart: nextSelectionStart,
    selectionEnd: nextSelectionEnd,
  };
}

export function replaceEmojiShortcodes(text: string): string {
  return replaceEmojiShortcodesWithSelection(text, null, null).text;
}

export function emojiNativeFromSelection(selection: unknown): string | null {
  if (typeof selection !== "object" || selection === null) {
    return null;
  }
  const native = (selection as { native?: unknown }).native;
  if (typeof native !== "string" || native.length === 0) {
    return null;
  }
  return native;
}

export function renderEmojiMixedText(text: string): (string | JSX.Element)[] {
  if (!text) {
    return [];
  }
  // Render inline as plain text to avoid runtime failures for unsupported emoji
  // sequences in emoji-mart's web component dataset.
  return [text];
}
