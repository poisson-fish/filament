import { createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  attachmentFromResponse,
  type AttachmentId,
  channelIdFromInput,
  guildIdFromInput,
  type AttachmentRecord,
} from "../src/domain/chat";
import { createAttachmentController } from "../src/features/app-shell/controllers/attachment-controller";
import { channelKey } from "../src/features/app-shell/helpers";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const UPLOADED_ATTACHMENT_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const RETAINED_ATTACHMENT_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

function attachmentFixture(input: {
  attachmentId: string;
  filename: string;
  mimeType: string;
  sizeBytes: number;
}): AttachmentRecord {
  return attachmentFromResponse({
    attachment_id: input.attachmentId,
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    owner_id: USER_ID,
    filename: input.filename,
    mime_type: input.mimeType,
    size_bytes: input.sizeBytes,
    sha256_hex: "a".repeat(64),
  });
}

describe("app shell attachment controller", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("runs upload, download, and delete flows against shared attachment state", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [selectedAttachment, setSelectedAttachment] = createSignal<File | null>(
      new File(["payload"], "incident.bin", {
        type: "application/octet-stream",
      }),
    );
    const [attachmentFilename, setAttachmentFilename] = createSignal("custom.bin");
    const [isUploadingAttachment, setUploadingAttachment] = createSignal(false);
    const [downloadingAttachmentId, setDownloadingAttachmentId] = createSignal<AttachmentId | null>(
      null,
    );
    const [deletingAttachmentId, setDeletingAttachmentId] = createSignal<AttachmentId | null>(
      null,
    );
    const [attachmentStatus, setAttachmentStatus] = createSignal("");
    const [attachmentError, setAttachmentError] = createSignal("");
    const [attachmentByChannel, setAttachmentByChannel] = createSignal<
      Record<string, AttachmentRecord[]>
    >({
      [channelKey(GUILD_ID, CHANNEL_ID)]: [
        attachmentFixture({
          attachmentId: UPLOADED_ATTACHMENT_ID,
          filename: "stale.bin",
          mimeType: "application/octet-stream",
          sizeBytes: 512,
        }),
        attachmentFixture({
          attachmentId: RETAINED_ATTACHMENT_ID,
          filename: "keep.bin",
          mimeType: "application/octet-stream",
          sizeBytes: 256,
        }),
      ],
    });

    const uploaded = attachmentFixture({
      attachmentId: UPLOADED_ATTACHMENT_ID,
      filename: "custom.bin",
      mimeType: "application/octet-stream",
      sizeBytes: 2048,
    });

    const uploadChannelAttachmentMock = vi.fn(async () => uploaded);
    const downloadChannelAttachmentMock = vi.fn(async () => ({
      bytes: new Uint8Array([1, 2, 3]),
      mimeType: "application/octet-stream",
    }));
    const deleteChannelAttachmentMock = vi.fn(async () => undefined);
    const createObjectUrlMock = vi.fn(() => "blob:download");
    const revokeObjectUrlMock = vi.fn();
    const clickSpy = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => undefined);

    const controller = createAttachmentController(
      {
        session,
        activeGuildId,
        activeChannelId,
        selectedAttachment,
        attachmentFilename,
        isUploadingAttachment,
        downloadingAttachmentId,
        deletingAttachmentId,
        setAttachmentStatus,
        setAttachmentError,
        setUploadingAttachment,
        setDownloadingAttachmentId,
        setDeletingAttachmentId,
        setSelectedAttachment,
        setAttachmentFilename,
        setAttachmentByChannel,
      },
      {
        uploadChannelAttachment: uploadChannelAttachmentMock,
        downloadChannelAttachment: downloadChannelAttachmentMock,
        deleteChannelAttachment: deleteChannelAttachmentMock,
        createObjectUrl: createObjectUrlMock,
        revokeObjectUrl: revokeObjectUrlMock,
      },
    );

    const preventDefault = vi.fn();
    await controller.uploadAttachment({
      preventDefault,
    } as unknown as SubmitEvent);

    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(uploadChannelAttachmentMock).toHaveBeenCalledTimes(1);
    expect(uploadChannelAttachmentMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      CHANNEL_ID,
      expect.any(File),
      "custom.bin",
    );
    expect(attachmentStatus()).toBe("Uploaded custom.bin (2.0 KiB).");
    expect(attachmentError()).toBe("");
    expect(selectedAttachment()).toBeNull();
    expect(attachmentFilename()).toBe("");
    expect(attachmentByChannel()[channelKey(GUILD_ID, CHANNEL_ID)]).toEqual([
      uploaded,
      attachmentFixture({
        attachmentId: RETAINED_ATTACHMENT_ID,
        filename: "keep.bin",
        mimeType: "application/octet-stream",
        sizeBytes: 256,
      }),
    ]);

    vi.useFakeTimers();
    await controller.downloadAttachment(uploaded);
    expect(downloadChannelAttachmentMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      CHANNEL_ID,
      uploaded.attachmentId,
    );
    expect(createObjectUrlMock).toHaveBeenCalledTimes(1);
    expect(clickSpy).toHaveBeenCalledTimes(1);
    vi.runAllTimers();
    expect(revokeObjectUrlMock).toHaveBeenCalledWith("blob:download");
    expect(downloadingAttachmentId()).toBeNull();

    await controller.removeAttachment(uploaded);
    expect(deleteChannelAttachmentMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      CHANNEL_ID,
      uploaded.attachmentId,
    );
    expect(attachmentByChannel()[channelKey(GUILD_ID, CHANNEL_ID)]).toEqual([
      attachmentFixture({
        attachmentId: RETAINED_ATTACHMENT_ID,
        filename: "keep.bin",
        mimeType: "application/octet-stream",
        sizeBytes: 256,
      }),
    ]);
    expect(attachmentStatus()).toBe("Deleted custom.bin.");
    expect(deletingAttachmentId()).toBeNull();
  });
});
