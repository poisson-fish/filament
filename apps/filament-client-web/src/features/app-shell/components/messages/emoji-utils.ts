import twitterData from "@emoji-mart/data/sets/14/twitter.json";
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
        init({ data: twitterData, set: "twitter", theme: "auto" });
        initialized = true;
    }
}

export function buildShortcodeMap() {
    if (shortcodeMap) return shortcodeMap;
    shortcodeMap = {};
    const data = twitterData as any;
    for (const emoji of Object.values(data.emojis) as any[]) {
        if (emoji.skins && emoji.skins[0] && emoji.skins[0].native) {
            const native = emoji.skins[0].native;
            shortcodeMap[`:${emoji.id}:`] = native;
            if (emoji.skins[0].shortcodes) {
                for (const sc of emoji.skins[0].shortcodes) {
                    shortcodeMap[sc] = native;
                }
            }
        }
    }
    return shortcodeMap;
}

export function replaceEmojiShortcodes(text: string): string {
    if (!text.includes(":")) return text;
    const map = buildShortcodeMap();
    return text.replace(/:([a-zA-Z0-9_+\-]+):/g, (match) => {
        return map[match] || match;
    });
}
