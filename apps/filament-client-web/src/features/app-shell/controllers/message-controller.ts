import type { AttachmentId, AttachmentRecord, MessageRecord } from "../../../domain/chat";
import { resolveAttachmentPreviewType } from "../helpers";

export function collectMediaPreviewTargets(
  messages: MessageRecord[],
  maxPreviewBytes: number,
): Map<AttachmentId, AttachmentRecord> {
  const previewTargets = new Map<AttachmentId, AttachmentRecord>();
  for (const message of messages) {
    for (const attachment of message.attachments) {
      const { kind } = resolveAttachmentPreviewType(
        null,
        attachment.mimeType,
        attachment.filename,
      );
      if (kind === "file" || attachment.sizeBytes > maxPreviewBytes) {
        continue;
      }
      previewTargets.set(attachment.attachmentId, attachment);
    }
  }
  return previewTargets;
}

export function retainRecordByAllowedIds<T>(
  existing: Record<string, T>,
  allowedIds: Set<string>,
): Record<string, T> {
  return Object.fromEntries(
    Object.entries(existing).filter(([id]) => allowedIds.has(id)),
  ) as Record<string, T>;
}

export function nextMediaPreviewAttempt(
  currentAttempts: ReadonlyMap<string, number>,
  attachmentId: string,
): number {
  return (currentAttempts.get(attachmentId) ?? 0) + 1;
}

export function shouldRetryMediaPreview(
  attempt: number,
  maxRetries: number,
): boolean {
  return attempt <= maxRetries;
}

export function mediaPreviewRetryDelayMs(
  attempt: number,
  baseDelayMs = 600,
): number {
  return baseDelayMs * Math.max(attempt, 1);
}
