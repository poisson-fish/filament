import { createLowlight } from "lowlight";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import plaintext from "highlight.js/lib/languages/plaintext";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

const LANGUAGE_LABEL_PATTERN = /^[a-z0-9_.+-]{1,32}$/;

const REGISTERED_LANGUAGE_GRAMMARS = {
  bash,
  c,
  cpp,
  css,
  go,
  java,
  javascript,
  json,
  markdown,
  plaintext,
  python,
  rust,
  sql,
  typescript,
  xml,
  yaml,
} as const;

export const ALLOWED_FENCED_CODE_LANGUAGES = Object.freeze(
  Object.keys(REGISTERED_LANGUAGE_GRAMMARS),
);

const LANGUAGE_ALIASES = Object.freeze({
  csharp: "plaintext",
  html: "xml",
  js: "javascript",
  plaintext: "plaintext",
  py: "python",
  rs: "rust",
  shell: "bash",
  sh: "bash",
  ts: "typescript",
} as const);

type RegisteredLanguage = (typeof ALLOWED_FENCED_CODE_LANGUAGES)[number];

export function createFilamentMarkdownHighlighter() {
  const lowlight = createLowlight();
  for (const [name, grammar] of Object.entries(REGISTERED_LANGUAGE_GRAMMARS)) {
    lowlight.register(name, grammar);
  }
  return lowlight;
}

export function resolveHighlightLanguage(
  rawLanguage: string | null,
): RegisteredLanguage | null {
  if (!rawLanguage) {
    return null;
  }
  const normalized = rawLanguage.trim().toLowerCase();
  if (!LANGUAGE_LABEL_PATTERN.test(normalized)) {
    return null;
  }
  const alias = LANGUAGE_ALIASES[normalized as keyof typeof LANGUAGE_ALIASES];
  if (alias) {
    return alias;
  }
  return ALLOWED_FENCED_CODE_LANGUAGES.includes(normalized)
    ? (normalized as RegisteredLanguage)
    : null;
}

