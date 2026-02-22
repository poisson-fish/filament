import twitterData from "@emoji-mart/data/sets/14/twitter.json";
import { init } from "emoji-mart";
import type { JSX } from "solid-js";

const TWEMOJI_SPRITESHEET_URL = new URL(
  // Keep this file aligned with Emoji Mart's expected twemoji sheet version; integrity is tested.
  "../../../../../resource/emoji/twitter-sheets-256-64.png",
  import.meta.url,
).href;

let initialized = false;
let shortcodeMap: Record<string, string> | null = null;
let twemojiByNative: Record<string, TwemojiSpriteCell> | null = null;
let twemojiSheetColumns: number | null = null;
let twemojiSheetRows: number | null = null;

const DEFAULT_INLINE_TWEMOJI_PX = 18;

export function initEmojiMart() {
  if (!initialized) {
    init({
      data: twitterData,
      set: "twitter",
      theme: "auto",
      getSpritesheetURL: () => TWEMOJI_SPRITESHEET_URL,
    });
    initialized = true;
  }
}

interface EmojiSkinRecord {
  native?: string;
  x?: number;
  y?: number;
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

interface TwemojiSpriteCell {
  x: number;
  y: number;
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

function ensureTwemojiMap(): Record<string, TwemojiSpriteCell> {
  if (twemojiByNative) {
    return twemojiByNative;
  }

  const map: Record<string, TwemojiSpriteCell> = {};
  const data = twitterData as EmojiDataRecord;
  const emojis = data.emojis ?? {};
  let maxX = 0;
  let maxY = 0;

  for (const emoji of Object.values(emojis)) {
    for (const skin of emoji.skins ?? []) {
      if (
        typeof skin.native !== "string" ||
        skin.native.length === 0 ||
        typeof skin.x !== "number" ||
        typeof skin.y !== "number"
      ) {
        continue;
      }
      map[skin.native] = { x: skin.x, y: skin.y };
      if (skin.x > maxX) {
        maxX = skin.x;
      }
      if (skin.y > maxY) {
        maxY = skin.y;
      }
    }
  }

  twemojiByNative = map;
  twemojiSheetColumns = maxX + 1;
  twemojiSheetRows = maxY + 1;
  return twemojiByNative;
}

function twemojiStyle(cell: TwemojiSpriteCell, sizePx: number): string {
  const columns = twemojiSheetColumns ?? 1;
  const rows = twemojiSheetRows ?? 1;
  const backgroundPosXPercent = columns > 1 ? (100 / (columns - 1)) * cell.x : 0;
  const backgroundPosYPercent = rows > 1 ? (100 / (rows - 1)) * cell.y : 0;
  return [
    `width:${sizePx}px`,
    `height:${sizePx}px`,
    `background-image:url("${TWEMOJI_SPRITESHEET_URL}")`,
    "background-repeat:no-repeat",
    `background-size:${100 * columns}% ${100 * rows}%`,
    `background-position:${backgroundPosXPercent}% ${backgroundPosYPercent}%`,
    "display:inline-block",
    "vertical-align:text-bottom",
  ].join(";");
}

export interface TwemojiRenderOptions {
  className?: string;
  sizePx?: number;
}

export function renderTwemojiNative(
  native: string,
  options: TwemojiRenderOptions = {},
): JSX.Element | string {
  const cell = ensureTwemojiMap()[native];
  if (!cell) {
    return native;
  }
  const sizePx = options.sizePx ?? DEFAULT_INLINE_TWEMOJI_PX;
  return (
    <span
      class={options.className ?? ""}
      role="img"
      aria-label={native}
      title={native}
      style={twemojiStyle(cell, sizePx)}
    />
  );
}

export function twemojiSpritesheetUrl(): string {
  return TWEMOJI_SPRITESHEET_URL;
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

const EMOJI_PATTERN =
  "(?:\\p{Regional_Indicator}{2}|[0-9#*]\\uFE0F?\\u20E3|\\p{Extended_Pictographic}(?:\\uFE0F|\\uFE0E)?(?:\\u200D\\p{Extended_Pictographic}(?:\\uFE0F|\\uFE0E)?)*)";
const EMOJI_SPLIT_REGEX = new RegExp(`(${EMOJI_PATTERN})`, "gu");
const EMOJI_TOKEN_REGEX = new RegExp(`^${EMOJI_PATTERN}$`, "u");

export function renderEmojiMixedText(text: string): (string | JSX.Element)[] {
  if (!text) {
    return [];
  }

  const parts = text.split(EMOJI_SPLIT_REGEX);
  return parts.map((part) => {
    if (!part) {
      return part;
    }
    if (!EMOJI_TOKEN_REGEX.test(part)) {
      return part;
    }
    return renderTwemojiNative(part, { className: "mb-[-0.2em]", sizePx: DEFAULT_INLINE_TWEMOJI_PX });
  });
}
