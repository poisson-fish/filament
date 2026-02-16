import { createSignal } from "solid-js";
import type {
  AttachmentId,
  AttachmentRecord,
  MessageId,
  MessageRecord,
} from "../../../domain/chat";
import type { ReactionView } from "../helpers";
import type { ReactionPickerOverlayPosition } from "../types";
import {
  createIdleAsyncOperationState,
  type AsyncOperationState,
} from "./async-operation-state";

export function createMessageState() {
  const [composer, setComposer] = createSignal("");
  const [messageStatus, setMessageStatus] = createSignal("");
  const [messageError, setMessageError] = createSignal("");
  const [isLoadingMessages, setLoadingMessages] = createSignal(false);
  const [isLoadingOlder, setLoadingOlder] = createSignal(false);
  const [isSendingMessage, setSendingMessage] = createSignal(false);
  const [sendMessageState, setSendMessageState] = createSignal<AsyncOperationState>(
    createIdleAsyncOperationState(),
  );
  const [refreshMessagesState, setRefreshMessagesState] =
    createSignal<AsyncOperationState>(createIdleAsyncOperationState());
  const [messages, setMessages] = createSignal<MessageRecord[]>([]);
  const [nextBefore, setNextBefore] = createSignal<MessageId | null>(null);
  const [showLoadOlderButton, setShowLoadOlderButton] = createSignal(false);

  const [reactionState, setReactionState] = createSignal<Record<string, ReactionView>>({});
  const [pendingReactionByKey, setPendingReactionByKey] = createSignal<Record<string, true>>({});
  const [openReactionPickerMessageId, setOpenReactionPickerMessageId] = createSignal<MessageId | null>(null);
  const [reactionPickerOverlayPosition, setReactionPickerOverlayPosition] =
    createSignal<ReactionPickerOverlayPosition | null>(null);

  const [editingMessageId, setEditingMessageId] = createSignal<MessageId | null>(null);
  const [editingDraft, setEditingDraft] = createSignal("");
  const [isSavingEdit, setSavingEdit] = createSignal(false);
  const [deletingMessageId, setDeletingMessageId] = createSignal<MessageId | null>(null);
  const [composerAttachments, setComposerAttachments] = createSignal<File[]>([]);

  const [attachmentByChannel, setAttachmentByChannel] = createSignal<Record<string, AttachmentRecord[]>>({});
  const [selectedAttachment, setSelectedAttachment] = createSignal<File | null>(null);
  const [attachmentFilename, setAttachmentFilename] = createSignal("");
  const [attachmentStatus, setAttachmentStatus] = createSignal("");
  const [attachmentError, setAttachmentError] = createSignal("");
  const [isUploadingAttachment, setUploadingAttachment] = createSignal(false);
  const [downloadingAttachmentId, setDownloadingAttachmentId] = createSignal<AttachmentId | null>(null);
  const [deletingAttachmentId, setDeletingAttachmentId] = createSignal<AttachmentId | null>(null);

  return {
    composer,
    setComposer,
    messageStatus,
    setMessageStatus,
    messageError,
    setMessageError,
    isLoadingMessages,
    setLoadingMessages,
    isLoadingOlder,
    setLoadingOlder,
    isSendingMessage,
    setSendingMessage,
    sendMessageState,
    setSendMessageState,
    refreshMessagesState,
    setRefreshMessagesState,
    messages,
    setMessages,
    nextBefore,
    setNextBefore,
    showLoadOlderButton,
    setShowLoadOlderButton,
    reactionState,
    setReactionState,
    pendingReactionByKey,
    setPendingReactionByKey,
    openReactionPickerMessageId,
    setOpenReactionPickerMessageId,
    reactionPickerOverlayPosition,
    setReactionPickerOverlayPosition,
    editingMessageId,
    setEditingMessageId,
    editingDraft,
    setEditingDraft,
    isSavingEdit,
    setSavingEdit,
    deletingMessageId,
    setDeletingMessageId,
    composerAttachments,
    setComposerAttachments,
    attachmentByChannel,
    setAttachmentByChannel,
    selectedAttachment,
    setSelectedAttachment,
    attachmentFilename,
    setAttachmentFilename,
    attachmentStatus,
    setAttachmentStatus,
    attachmentError,
    setAttachmentError,
    isUploadingAttachment,
    setUploadingAttachment,
    downloadingAttachmentId,
    setDownloadingAttachmentId,
    deletingAttachmentId,
    setDeletingAttachmentId,
  };
}
