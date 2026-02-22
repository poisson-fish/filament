import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const TWEMOJI_SPRITESHEET_PATH = join(process.cwd(), "resource/emoji/twitter-sheets-256-64.png");
const EXPECTED_TWEMOJI_15_0_1_SHA256 =
  "1a2c23876eed03dca384bd58868e106c298a7bd9e83714c8da94562a5a3280f7";

describe("emoji spritesheet integrity", () => {
  it("pins the bundled twemoji sheet to the expected Emoji Mart-compatible build", () => {
    const digest = createHash("sha256")
      .update(readFileSync(TWEMOJI_SPRITESHEET_PATH))
      .digest("hex");
    expect(digest).toBe(EXPECTED_TWEMOJI_15_0_1_SHA256);
  });
});
