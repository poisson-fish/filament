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
  const panelSectionClass = "grid gap-[0.5rem]";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const submitButtonClass =
    "min-h-[1.95rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";
  const presenceDotClass = "inline-block h-[0.58rem] w-[0.58rem] rounded-full";
  const onlinePresenceDotClass = `${presenceDotClass} bg-presence-online`;
  const idlePresenceDotClass = `${presenceDotClass} bg-presence-idle`;
  const attachmentListClass = "m-0 grid list-none gap-[0.42rem] p-0";
  const attachmentListItemClass =
    "grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-center gap-[0.45rem] overflow-hidden rounded-[0.6rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.5rem]";
  const attachmentNameClass = "min-w-0 break-words text-[0.84rem] text-ink-0";
  const attachmentActionButtonClass =
    "min-h-[1.75rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.54rem] text-[0.78rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const metadataTextClass = "text-[0.78rem] text-ink-2 font-code";

  return (
    <section class={panelSectionClass}>
      <form class={formClass} onSubmit={props.onSubmitUpload}>
        <label class={fieldLabelClass}>
          File
          <input
            class={fieldControlClass}
            type="file"
            onInput={(event) => {
              const file = event.currentTarget.files?.[0] ?? null;
              props.onAttachmentFileInput(file);
            }}
          />
        </label>
        <label class={fieldLabelClass}>
          Filename
          <input
            class={fieldControlClass}
            value={props.attachmentFilename}
            onInput={(event) => props.onAttachmentFilenameInput(event.currentTarget.value)}
            maxlength="128"
            placeholder="upload.bin"
          />
        </label>
        <button
          class={submitButtonClass}
          type="submit"
          disabled={props.isUploadingAttachment || !props.hasActiveChannel}
        >
          {props.isUploadingAttachment ? "Uploading..." : "Upload"}
        </button>
      </form>
      <Show when={props.attachmentStatus}>
        <p class={statusOkClass}>{props.attachmentStatus}</p>
      </Show>
      <Show when={props.attachmentError}>
        <p class={statusErrorClass}>{props.attachmentError}</p>
      </Show>
      <ul class={attachmentListClass}>
        <For each={props.activeAttachments}>
          {(record) => (
            <li class={attachmentListItemClass}>
              <span class={onlinePresenceDotClass} />
              <div class="grid min-w-0 gap-[0.16rem]">
                <span class={attachmentNameClass}>{record.filename}</span>
                <span class={metadataTextClass}>
                  {record.mimeType} · {formatBytes(record.sizeBytes)}
                </span>
              </div>
              <button
                type="button"
                class={attachmentActionButtonClass}
                onClick={() => void props.onDownloadAttachment(record)}
                disabled={props.downloadingAttachmentId === record.attachmentId}
              >
                {props.downloadingAttachmentId === record.attachmentId ? "..." : "Get"}
              </button>
              <button
                type="button"
                class={attachmentActionButtonClass}
                onClick={() => void props.onRemoveAttachment(record)}
                disabled={props.deletingAttachmentId === record.attachmentId}
              >
                {props.deletingAttachmentId === record.attachmentId ? "..." : "Del"}
              </button>
            </li>
          )}
        </For>
        <Show when={props.activeAttachments.length === 0}>
          <li class={attachmentListItemClass}>
            <span class={idlePresenceDotClass} />
            <span class="col-span-3 min-w-0 break-words text-[0.84rem] text-ink-2">
              no-local-attachments
            </span>
          </li>
        </Show>
      </ul>
    </section>
  );
}
