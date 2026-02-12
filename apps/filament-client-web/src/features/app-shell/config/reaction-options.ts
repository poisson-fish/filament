import { reactionEmojiFromInput } from "../../../domain/chat";
import type { ReactionPickerOption } from "../types";

export const OPENMOJI_REACTION_OPTIONS: ReactionPickerOption[] = [
  {
    emoji: reactionEmojiFromInput("👍"),
    label: "Thumbs up",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F44D.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👎"),
    label: "Thumbs down",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F44E.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😂"),
    label: "Tears of joy",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F602.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤣"),
    label: "Rolling on the floor laughing",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F923.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😮"),
    label: "Surprised",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F62E.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😢"),
    label: "Crying",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F622.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😱"),
    label: "Screaming",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F631.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👏"),
    label: "Clapping",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F44F.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🔥"),
    label: "Fire",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F525.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🎉"),
    label: "Party popper",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F389.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤔"),
    label: "Thinking",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F914.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🙌"),
    label: "Raised hands",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F64C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🚀"),
    label: "Rocket",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F680.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💯"),
    label: "Hundred points",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F4AF.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🏆"),
    label: "Trophy",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F3C6.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤝"),
    label: "Handshake",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F91D.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🙏"),
    label: "Folded hands",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F64F.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👌"),
    label: "Ok hand",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F44C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("✅"),
    label: "Check mark",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/2705.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("❌"),
    label: "Cross mark",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/274C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("❤"),
    label: "Heart",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/2764.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💜"),
    label: "Purple heart",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F49C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🧠"),
    label: "Brain",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F9E0.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💡"),
    label: "Light bulb",
    iconUrl: new URL("../../../../resource/openmoji-svg-color/1F4A1.svg", import.meta.url).href,
  },
];
