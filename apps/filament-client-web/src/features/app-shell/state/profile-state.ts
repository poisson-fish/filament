import { createSignal } from "solid-js";
import type { UserId } from "../../../domain/chat";

export function createProfileState() {
  const [gatewayOnline, setGatewayOnline] = createSignal(false);
  const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);
  const [resolvedUsernames, setResolvedUsernames] = createSignal<Record<string, string>>({});
  const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<Record<string, number>>({});
  const [bannerVersionByUserId, setBannerVersionByUserId] = createSignal<Record<string, number>>({});

  const [profileDraftUsername, setProfileDraftUsername] = createSignal("");
  const [profileDraftAbout, setProfileDraftAbout] = createSignal("");
  const [selectedProfileAvatarFile, setSelectedProfileAvatarFile] = createSignal<File | null>(null);
  const [selectedProfileBannerFile, setSelectedProfileBannerFile] = createSignal<File | null>(null);
  const [profileSettingsStatus, setProfileSettingsStatus] = createSignal("");
  const [profileSettingsError, setProfileSettingsError] = createSignal("");
  const [isSavingProfile, setSavingProfile] = createSignal(false);
  const [isUploadingProfileAvatar, setUploadingProfileAvatar] = createSignal(false);
  const [isUploadingProfileBanner, setUploadingProfileBanner] = createSignal(false);
  const [selectedProfileUserId, setSelectedProfileUserId] = createSignal<UserId | null>(null);
  const [selectedProfileError, setSelectedProfileError] = createSignal("");

  return {
    gatewayOnline,
    setGatewayOnline,
    onlineMembers,
    setOnlineMembers,
    resolvedUsernames,
    setResolvedUsernames,
    avatarVersionByUserId,
    setAvatarVersionByUserId,
    bannerVersionByUserId,
    setBannerVersionByUserId,
    profileDraftUsername,
    setProfileDraftUsername,
    profileDraftAbout,
    setProfileDraftAbout,
    selectedProfileAvatarFile,
    setSelectedProfileAvatarFile,
    selectedProfileBannerFile,
    setSelectedProfileBannerFile,
    profileSettingsStatus,
    setProfileSettingsStatus,
    profileSettingsError,
    setProfileSettingsError,
    isSavingProfile,
    setSavingProfile,
    isUploadingProfileAvatar,
    setUploadingProfileAvatar,
    isUploadingProfileBanner,
    setUploadingProfileBanner,
    selectedProfileUserId,
    setSelectedProfileUserId,
    selectedProfileError,
    setSelectedProfileError,
  };
}
