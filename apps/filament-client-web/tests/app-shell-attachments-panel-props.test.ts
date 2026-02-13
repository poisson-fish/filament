import { describe, expect, it, vi } from "vitest";
import {
  attachmentFromResponse,
  attachmentIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
} from "../src/domain/chat";
import { createAttachmentsPanelProps } from "../src/features/app-shell/runtime/attachments-panel-props";

describe("app shell attachments panel props", () => {
  it("maps attachment values and handlers", async () => {
    const setSelectedAttachment = vi.fn();
    const setAttachmentFilename = vi.fn();
    const onSubmitUploadAttachment = vi.fn();
    const onDownloadAttachment = vi.fn();
    const onRemoveAttachment = vi.fn();

    const attachment = attachmentFromResponse({
      attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FA1",
      guild_id: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA2"),
      channel_id: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA3"),
      owner_id: "01ARZ3NDEKTSV4RRFFQ69G5FA4",
      filename: "incident.log",
      mime_type: "text/plain",
      size_bytes: 128,
      sha256_hex: "a".repeat(64),
    });

    const panelProps = createAttachmentsPanelProps({
      attachmentFilename: "incident.log",
      activeAttachments: [attachment],
      isUploadingAttachment: false,
      hasActiveChannel: true,
      attachmentStatus: "ready",
      attachmentError: "",
      downloadingAttachmentId: attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA5"),
      deletingAttachmentId: null,
      onSubmitUploadAttachment,
      setSelectedAttachment,
      setAttachmentFilename,
      onDownloadAttachment,
      onRemoveAttachment,
    });

    expect(panelProps.attachmentFilename).toBe("incident.log");
    expect(panelProps.activeAttachments).toHaveLength(1);
    expect(panelProps.hasActiveChannel).toBe(true);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onSubmitUploadAttachment(submitEvent);
    expect(onSubmitUploadAttachment).toHaveBeenCalledWith(submitEvent);

    panelProps.setAttachmentFilename("incident-2.log");
    expect(setAttachmentFilename).toHaveBeenCalledWith("incident-2.log");

    panelProps.setSelectedAttachment(null);
    expect(setSelectedAttachment).toHaveBeenCalledWith(null);

    await panelProps.onDownloadAttachment(attachment);
    expect(onDownloadAttachment).toHaveBeenCalledWith(attachment);

    await panelProps.onRemoveAttachment(attachment);
    expect(onRemoveAttachment).toHaveBeenCalledWith(attachment);
  });
});
