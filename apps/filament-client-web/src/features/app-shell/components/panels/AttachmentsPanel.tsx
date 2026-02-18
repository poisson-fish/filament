import { For, Show } from "solid-js";
import type { AttachmentId, AttachmentRecord } from "../../../../domain/chat";
import { formatBytes } from "../../helpers";

export interface AttachmentsPanelProps {
  attachmentFilename: string;
  activeAttachments: AttachmentRecord[];
  isUploadingAttachment: boolean;
  hasActiveChannel: boolean;
  attachmentStatus: string;
  attachmentError: string;
  downloadingAttachmentId: AttachmentId | null;
  deletingAttachmentId: AttachmentId | null;
  onSubmitUpload: (event: SubmitEvent) => Promise<void> | void;
  onAttachmentFileInput: (file: File | null) => void;
  onAttachmentFilenameInput: (value: string) => void;
  onDownloadAttachment: (record: AttachmentRecord) => Promise<void> | void;
  onRemoveAttachment: (record: AttachmentRecord) => Promise<void> | void;
}

export function AttachmentsPanel(props: AttachmentsPanelProps) {
  return (
    <section class="member-group">
      <form class="inline-form" onSubmit={props.onSubmitUpload}>
        <label>
          File
          <input
            type="file"
            onInput={(event) => {
              const file = event.currentTarget.files?.[0] ?? null;
              props.onAttachmentFileInput(file);
            }}
          />
        </label>
        <label>
          Filename
          <input
            value={props.attachmentFilename}
            onInput={(event) => props.onAttachmentFilenameInput(event.currentTarget.value)}
            maxlength="128"
            placeholder="upload.bin"
          />
        </label>
        <button type="submit" disabled={props.isUploadingAttachment || !props.hasActiveChannel}>
          {props.isUploadingAttachment ? "Uploading..." : "Upload"}
        </button>
      </form>
      <Show when={props.attachmentStatus}>
        <p class="status ok">{props.attachmentStatus}</p>
      </Show>
      <Show when={props.attachmentError}>
        <p class="status error">{props.attachmentError}</p>
      </Show>
      <ul>
        <For each={props.activeAttachments}>
          {(record) => (
            <li>
              <span class="presence online" />
              <div class="grid min-w-0 gap-[0.16rem]">
                <span>{record.filename}</span>
                <span class="muted text-[0.78rem] font-code">
                  {record.mimeType} · {formatBytes(record.sizeBytes)}
                </span>
              </div>
              <button
                type="button"
                onClick={() => void props.onDownloadAttachment(record)}
                disabled={props.downloadingAttachmentId === record.attachmentId}
              >
                {props.downloadingAttachmentId === record.attachmentId ? "..." : "Get"}
              </button>
              <button
                type="button"
                onClick={() => void props.onRemoveAttachment(record)}
                disabled={props.deletingAttachmentId === record.attachmentId}
              >
                {props.deletingAttachmentId === record.attachmentId ? "..." : "Del"}
              </button>
            </li>
          )}
        </For>
        <Show when={props.activeAttachments.length === 0}>
          <li>
            <span class="presence idle" />
            no-local-attachments
          </li>
        </Show>
      </ul>
    </section>
  );
}
