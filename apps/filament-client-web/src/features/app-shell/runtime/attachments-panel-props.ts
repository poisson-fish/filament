import type {
  AttachmentsPanelBuilderOptions,
} from "../adapters/panel-host-props";

export interface AttachmentsPanelPropsOptions {
  attachmentFilename: string;
  activeAttachments: AttachmentsPanelBuilderOptions["activeAttachments"];
  isUploadingAttachment: boolean;
  hasActiveChannel: boolean;
  attachmentStatus: string;
  attachmentError: string;
  downloadingAttachmentId: AttachmentsPanelBuilderOptions["downloadingAttachmentId"];
  deletingAttachmentId: AttachmentsPanelBuilderOptions["deletingAttachmentId"];
  onSubmitUploadAttachment: (event: SubmitEvent) => Promise<void> | void;
  setSelectedAttachment: (file: File | null) => void;
  setAttachmentFilename: (value: string) => void;
  onDownloadAttachment: (record: AttachmentsPanelBuilderOptions["activeAttachments"][number]) =>
    Promise<void> | void;
  onRemoveAttachment: (record: AttachmentsPanelBuilderOptions["activeAttachments"][number]) =>
    Promise<void> | void;
}

export function createAttachmentsPanelProps(
  options: AttachmentsPanelPropsOptions,
): AttachmentsPanelBuilderOptions {
  return {
    attachmentFilename: options.attachmentFilename,
    activeAttachments: options.activeAttachments,
    isUploadingAttachment: options.isUploadingAttachment,
    hasActiveChannel: options.hasActiveChannel,
    attachmentStatus: options.attachmentStatus,
    attachmentError: options.attachmentError,
    downloadingAttachmentId: options.downloadingAttachmentId,
    deletingAttachmentId: options.deletingAttachmentId,
    onSubmitUploadAttachment: options.onSubmitUploadAttachment,
    setSelectedAttachment: options.setSelectedAttachment,
    setAttachmentFilename: options.setAttachmentFilename,
    onDownloadAttachment: (record) => options.onDownloadAttachment(record),
    onRemoveAttachment: (record) => options.onRemoveAttachment(record),
  };
}
