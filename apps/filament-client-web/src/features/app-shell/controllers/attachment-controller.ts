import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  attachmentFilenameFromInput,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelId,
  type GuildId,
} from "../../../domain/chat";
import {
  deleteChannelAttachment,
  downloadChannelAttachment,
  uploadChannelAttachment,
} from "../../../lib/api";
import {
  channelKey,
  createObjectUrl,
  formatBytes,
  mapError,
  revokeObjectUrl,
} from "../helpers";

export interface AttachmentControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  selectedAttachment: Accessor<File | null>;
  attachmentFilename: Accessor<string>;
  isUploadingAttachment: Accessor<boolean>;
  downloadingAttachmentId: Accessor<AttachmentId | null>;
  deletingAttachmentId: Accessor<AttachmentId | null>;
  setAttachmentStatus: Setter<string>;
  setAttachmentError: Setter<string>;
  setUploadingAttachment: Setter<boolean>;
  setDownloadingAttachmentId: Setter<AttachmentId | null>;
  setDeletingAttachmentId: Setter<AttachmentId | null>;
  setSelectedAttachment: Setter<File | null>;
  setAttachmentFilename: Setter<string>;
  setAttachmentByChannel: Setter<Record<string, AttachmentRecord[]>>;
}

export interface AttachmentControllerDependencies {
  uploadChannelAttachment: typeof uploadChannelAttachment;
  downloadChannelAttachment: typeof downloadChannelAttachment;
  deleteChannelAttachment: typeof deleteChannelAttachment;
  createObjectUrl: (blob: Blob) => string | null;
  revokeObjectUrl: (url: string) => void;
}

export interface AttachmentController {
  uploadAttachment: (event: SubmitEvent) => Promise<void>;
  downloadAttachment: (record: AttachmentRecord) => Promise<void>;
  removeAttachment: (record: AttachmentRecord) => Promise<void>;
}

const DEFAULT_ATTACHMENT_CONTROLLER_DEPENDENCIES: AttachmentControllerDependencies = {
  uploadChannelAttachment,
  downloadChannelAttachment,
  deleteChannelAttachment,
  createObjectUrl,
  revokeObjectUrl,
};

export function createAttachmentController(
  options: AttachmentControllerOptions,
  dependencies: Partial<AttachmentControllerDependencies> = {},
): AttachmentController {
  const deps = {
    ...DEFAULT_ATTACHMENT_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const uploadAttachment = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    const file = options.selectedAttachment();
    if (!session || !guildId || !channelId) {
      options.setAttachmentError("Select a channel first.");
      return;
    }
    if (!file) {
      options.setAttachmentError("Select a file to upload.");
      return;
    }
    if (options.isUploadingAttachment()) {
      return;
    }

    options.setAttachmentStatus("");
    options.setAttachmentError("");
    options.setUploadingAttachment(true);

    try {
      const filename = attachmentFilenameFromInput(
        options.attachmentFilename().trim().length > 0
          ? options.attachmentFilename().trim()
          : file.name,
      );
      const uploaded = await deps.uploadChannelAttachment(
        session,
        guildId,
        channelId,
        file,
        filename,
      );
      const key = channelKey(guildId, channelId);
      options.setAttachmentByChannel((existing) => {
        const current = existing[key] ?? [];
        const deduped = current.filter(
          (entry) => entry.attachmentId !== uploaded.attachmentId,
        );
        return {
          ...existing,
          [key]: [uploaded, ...deduped],
        };
      });
      options.setAttachmentStatus(
        `Uploaded ${uploaded.filename} (${formatBytes(uploaded.sizeBytes)}).`,
      );
      options.setSelectedAttachment(null);
      options.setAttachmentFilename("");
    } catch (error) {
      options.setAttachmentError(mapError(error, "Unable to upload attachment."));
    } finally {
      options.setUploadingAttachment(false);
    }
  };

  const downloadAttachment = async (record: AttachmentRecord) => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || options.downloadingAttachmentId()) {
      return;
    }

    options.setDownloadingAttachmentId(record.attachmentId);
    options.setAttachmentError("");
    try {
      const payload = await deps.downloadChannelAttachment(
        session,
        guildId,
        channelId,
        record.attachmentId,
      );
      const blob = new Blob([payload.bytes.buffer as ArrayBuffer], {
        type: payload.mimeType ?? record.mimeType,
      });
      const objectUrl = deps.createObjectUrl(blob);
      if (!objectUrl) {
        throw new Error("missing_object_url");
      }
      const anchor = document.createElement("a");
      anchor.href = objectUrl;
      anchor.download = record.filename;
      anchor.rel = "noopener";
      anchor.click();
      window.setTimeout(() => deps.revokeObjectUrl(objectUrl), 0);
    } catch (error) {
      options.setAttachmentError(mapError(error, "Unable to download attachment."));
    } finally {
      options.setDownloadingAttachmentId(null);
    }
  };

  const removeAttachment = async (record: AttachmentRecord) => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || options.deletingAttachmentId()) {
      return;
    }

    options.setDeletingAttachmentId(record.attachmentId);
    options.setAttachmentError("");
    try {
      await deps.deleteChannelAttachment(
        session,
        guildId,
        channelId,
        record.attachmentId,
      );
      const key = channelKey(guildId, channelId);
      options.setAttachmentByChannel((existing) => ({
        ...existing,
        [key]: (existing[key] ?? []).filter(
          (entry) => entry.attachmentId !== record.attachmentId,
        ),
      }));
      options.setAttachmentStatus(`Deleted ${record.filename}.`);
    } catch (error) {
      options.setAttachmentError(mapError(error, "Unable to delete attachment."));
    } finally {
      options.setDeletingAttachmentId(null);
    }
  };

  return {
    uploadAttachment,
    downloadAttachment,
    removeAttachment,
  };
}
