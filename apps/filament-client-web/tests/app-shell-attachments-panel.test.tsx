import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  attachmentFromResponse,
  attachmentIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
  userIdFromInput,
} from "../src/domain/chat";
import {
  AttachmentsPanel,
  type AttachmentsPanelProps,
} from "../src/features/app-shell/components/panels/AttachmentsPanel";

const ATTACHMENT = attachmentFromResponse({
  attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FA1",
  guild_id: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA2"),
  channel_id: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA3"),
  owner_id: userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA4"),
  filename: "incident.log",
  mime_type: "text/plain",
  size_bytes: 128,
  sha256_hex: "a".repeat(64),
});

function attachmentsPanelPropsFixture(
  overrides: Partial<AttachmentsPanelProps> = {},
): AttachmentsPanelProps {
  return {
    attachmentFilename: "incident.log",
    activeAttachments: [ATTACHMENT],
    isUploadingAttachment: false,
    hasActiveChannel: true,
    attachmentStatus: "",
    attachmentError: "",
    downloadingAttachmentId: null,
    deletingAttachmentId: null,
    onSubmitUpload: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onAttachmentFileInput: vi.fn(),
    onAttachmentFilenameInput: vi.fn(),
    onDownloadAttachment: vi.fn(),
    onRemoveAttachment: vi.fn(),
    ...overrides,
  };
}

describe("app shell attachments panel", () => {
  it("renders metadata with utility classes and without legacy stacked-meta/mono hooks", () => {
    render(() => <AttachmentsPanel {...attachmentsPanelPropsFixture()} />);

    expect(screen.getByText("incident.log")).toBeInTheDocument();
    const metadata = screen.getByText("text/plain · 128 B");
    expect(metadata).toHaveClass("font-code");
    expect(metadata).toHaveClass("text-[0.78rem]");
    expect(document.querySelector(".stacked-meta")).toBeNull();
    expect(document.querySelector(".mono")).toBeNull();
  });

  it("keeps upload/download/remove handlers wired", async () => {
    const onSubmitUpload = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onAttachmentFileInput = vi.fn();
    const onAttachmentFilenameInput = vi.fn();
    const onDownloadAttachment = vi.fn();
    const onRemoveAttachment = vi.fn();

    render(() => (
      <AttachmentsPanel
        {...attachmentsPanelPropsFixture({
          onSubmitUpload,
          onAttachmentFileInput,
          onAttachmentFilenameInput,
          onDownloadAttachment,
          onRemoveAttachment,
          downloadingAttachmentId: attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA9"),
          deletingAttachmentId: attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA"),
        })}
      />
    ));

    const uploadForm = screen.getByRole("button", { name: "Upload" }).closest("form");
    expect(uploadForm).not.toBeNull();
    await fireEvent.submit(uploadForm!);
    expect(onSubmitUpload).toHaveBeenCalledTimes(1);

    await fireEvent.input(screen.getByLabelText("Filename"), {
      target: { value: "incident-updated.log" },
    });
    expect(onAttachmentFilenameInput).toHaveBeenCalledWith("incident-updated.log");

    const fileInput = screen.getByLabelText("File") as HTMLInputElement;
    const file = new File(["bytes"], "incident.log", { type: "text/plain" });
    Object.defineProperty(fileInput, "files", {
      configurable: true,
      value: [file],
    });
    await fireEvent.input(fileInput);
    expect(onAttachmentFileInput).toHaveBeenCalledWith(file);

    await fireEvent.click(screen.getByRole("button", { name: "Get" }));
    expect(onDownloadAttachment).toHaveBeenCalledWith(ATTACHMENT);

    await fireEvent.click(screen.getByRole("button", { name: "Del" }));
    expect(onRemoveAttachment).toHaveBeenCalledWith(ATTACHMENT);
  });
});
