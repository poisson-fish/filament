use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use filament_core::UserId;
use serde::Deserialize;
use ulid::Ulid;

pub const DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP: u32 = 20;
pub const DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER: u32 = 10;
pub const DEFAULT_AUDIT_LIST_LIMIT_MAX: usize = 100;
pub const DEFAULT_GUILD_IP_BAN_MAX_ENTRIES: usize = 4_096;

pub const DEFAULT_AUDIT_LIST_LIMIT: usize = 20;
pub const DEFAULT_GUILD_IP_BAN_LIST_LIMIT: usize = 20;
pub const DEFAULT_GUILD_IP_BAN_LIST_LIMIT_MAX: usize = 100;
pub const MAX_AUDIT_ACTION_PREFIX_CHARS: usize = 64;
pub const MAX_GUILD_IP_BAN_REASON_CHARS: usize = 240;
pub const MAX_GUILD_IP_BAN_EXPIRY_SECS: u64 = 60 * 60 * 24 * 180;
pub const MAX_AUDIT_CURSOR_CHARS: usize = 128;
pub const MAX_WORKSPACE_ROLE_NAME_CHARS: usize = 32;

pub const DIRECTORY_JOIN_NOT_ALLOWED_ERROR: &str = "directory_join_not_allowed";
pub const DIRECTORY_JOIN_USER_BANNED_ERROR: &str = "directory_join_user_banned";
pub const DIRECTORY_JOIN_IP_BANNED_ERROR: &str = "directory_join_ip_banned";
pub const AUDIT_ACCESS_DENIED_ERROR: &str = "audit_access_denied";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryJoinOutcome {
    Accepted,
    AlreadyMember,
    RejectedVisibility,
    RejectedUserBan,
    RejectedIpBan,
}

impl DirectoryJoinOutcome {
    #[must_use]
    pub const fn audit_action(self) -> &'static str {
        match self {
            Self::Accepted | Self::AlreadyMember => "directory.join.accepted",
            Self::RejectedVisibility => "directory.join.rejected.visibility",
            Self::RejectedUserBan => "directory.join.rejected.user_ban",
            Self::RejectedIpBan => "directory.join.rejected.ip_ban",
        }
    }

    #[must_use]
    pub const fn rejection_error(self) -> Option<&'static str> {
        match self {
            Self::RejectedVisibility => Some(DIRECTORY_JOIN_NOT_ALLOWED_ERROR),
            Self::RejectedUserBan => Some(DIRECTORY_JOIN_USER_BANNED_ERROR),
            Self::RejectedIpBan => Some(DIRECTORY_JOIN_IP_BANNED_ERROR),
            Self::Accepted | Self::AlreadyMember => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuildIpBanId(Ulid);

impl GuildIpBanId {
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for GuildIpBanId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for GuildIpBanId {
    type Error = DirectoryContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| DirectoryContractError::Id)?;
        Ok(Self(parsed))
    }
}

impl std::fmt::Display for GuildIpBanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceRoleId(Ulid);

impl WorkspaceRoleId {
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for WorkspaceRoleId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for WorkspaceRoleId {
    type Error = DirectoryContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| DirectoryContractError::RoleId)?;
        Ok(Self(parsed))
    }
}

impl std::fmt::Display for WorkspaceRoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn validate_workspace_role_name(value: &str) -> Result<String, DirectoryContractError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_WORKSPACE_ROLE_NAME_CHARS {
        return Err(DirectoryContractError::RoleName);
    }
    if normalized
        .chars()
        .any(|char| char.is_ascii_control() || char == '\u{7f}')
    {
        return Err(DirectoryContractError::RoleName);
    }
    Ok(normalized.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpNetwork {
    network: IpAddr,
    prefix: u8,
}

impl IpNetwork {
    #[must_use]
    pub fn host(ip: IpAddr) -> Self {
        let prefix = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        Self {
            network: ip,
            prefix,
        }
    }

    #[must_use]
    pub fn network(self) -> IpAddr {
        self.network
    }

    #[must_use]
    pub fn prefix(self) -> u8 {
        self.prefix
    }

    #[must_use]
    pub fn contains(self, ip: IpAddr) -> bool {
        canonicalize_ip(ip, self.prefix)
            .map(|value| value == self.network)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn canonical_cidr(self) -> String {
        format!("{}/{}", self.network, self.prefix)
    }
}

impl FromStr for IpNetwork {
    type Err = DirectoryContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.trim().is_empty() {
            return Err(DirectoryContractError::IpNetwork);
        }
        let (address, prefix) = if let Some((ip_part, prefix_part)) = value.split_once('/') {
            if ip_part.is_empty() || prefix_part.is_empty() || prefix_part.contains('/') {
                return Err(DirectoryContractError::IpNetwork);
            }
            let ip = IpAddr::from_str(ip_part).map_err(|_| DirectoryContractError::IpNetwork)?;
            let parsed_prefix = prefix_part
                .parse::<u8>()
                .map_err(|_| DirectoryContractError::IpNetwork)?;
            (ip, parsed_prefix)
        } else {
            let ip = IpAddr::from_str(value).map_err(|_| DirectoryContractError::IpNetwork)?;
            let host_prefix = match ip {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };
            (ip, host_prefix)
        };

        let canonical_ip = canonicalize_ip(address, prefix)?;
        Ok(Self {
            network: canonical_ip,
            prefix,
        })
    }
}

impl TryFrom<String> for IpNetwork {
    type Error = DirectoryContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl std::fmt::Display for IpNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditCursor(String);

impl AuditCursor {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AuditCursor {
    type Error = DirectoryContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > MAX_AUDIT_CURSOR_CHARS {
            return Err(DirectoryContractError::AuditCursor);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(DirectoryContractError::AuditCursor);
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditListQueryDto {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    pub action_prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditListQuery {
    pub cursor: Option<AuditCursor>,
    pub limit: usize,
    pub action_prefix: Option<String>,
}

impl TryFrom<AuditListQueryDto> for AuditListQuery {
    type Error = DirectoryContractError;

    fn try_from(value: AuditListQueryDto) -> Result<Self, Self::Error> {
        Self::try_from_with_limit_max(value, DEFAULT_AUDIT_LIST_LIMIT_MAX)
    }
}

impl AuditListQuery {
    /// Parse and validate an audit-list query while applying a runtime-configured `limit` cap.
    ///
    /// # Errors
    /// Returns `DirectoryContractError` when cursor, limit, or action-prefix invariants are not met.
    pub fn try_from_with_limit_max(
        value: AuditListQueryDto,
        limit_max: usize,
    ) -> Result<Self, DirectoryContractError> {
        let limit = value.limit.unwrap_or(DEFAULT_AUDIT_LIST_LIMIT);
        if limit == 0 || limit > limit_max {
            return Err(DirectoryContractError::Limit);
        }

        let action_prefix = match value.action_prefix {
            None => None,
            Some(prefix) => {
                if prefix.is_empty()
                    || prefix.len() > MAX_AUDIT_ACTION_PREFIX_CHARS
                    || !prefix.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'_')
                    })
                {
                    return Err(DirectoryContractError::ActionPrefix);
                }
                Some(prefix)
            }
        };

        Ok(Self {
            cursor: value.cursor.map(AuditCursor::try_from).transpose()?,
            limit,
            action_prefix,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildIpBanListQueryDto {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildIpBanListQuery {
    pub cursor: Option<AuditCursor>,
    pub limit: usize,
}

impl TryFrom<GuildIpBanListQueryDto> for GuildIpBanListQuery {
    type Error = DirectoryContractError;

    fn try_from(value: GuildIpBanListQueryDto) -> Result<Self, Self::Error> {
        let limit = value.limit.unwrap_or(DEFAULT_GUILD_IP_BAN_LIST_LIMIT);
        if limit == 0 || limit > DEFAULT_GUILD_IP_BAN_LIST_LIMIT_MAX {
            return Err(DirectoryContractError::Limit);
        }

        Ok(Self {
            cursor: value.cursor.map(AuditCursor::try_from).transpose()?,
            limit,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildIpBanByUserRequestDto {
    pub target_user_id: String,
    pub reason: Option<String>,
    pub expires_in_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildIpBanByUserRequest {
    pub target_user_id: UserId,
    pub reason: Option<String>,
    pub expires_in_secs: Option<u64>,
}

impl TryFrom<GuildIpBanByUserRequestDto> for GuildIpBanByUserRequest {
    type Error = DirectoryContractError;

    fn try_from(value: GuildIpBanByUserRequestDto) -> Result<Self, Self::Error> {
        let target_user_id =
            UserId::try_from(value.target_user_id).map_err(|_| DirectoryContractError::UserId)?;

        let reason = match value.reason {
            None => None,
            Some(text) => {
                if text.trim().is_empty() || text.len() > MAX_GUILD_IP_BAN_REASON_CHARS {
                    return Err(DirectoryContractError::Reason);
                }
                Some(text)
            }
        };

        if value
            .expires_in_secs
            .is_some_and(|secs| secs == 0 || secs > MAX_GUILD_IP_BAN_EXPIRY_SECS)
        {
            return Err(DirectoryContractError::Expiry);
        }

        Ok(Self {
            target_user_id,
            reason,
            expires_in_secs: value.expires_in_secs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryContractError {
    Id,
    RoleId,
    RoleName,
    IpNetwork,
    AuditCursor,
    Limit,
    ActionPrefix,
    UserId,
    Reason,
    Expiry,
}

fn canonicalize_ip(ip: IpAddr, prefix: u8) -> Result<IpAddr, DirectoryContractError> {
    match ip {
        IpAddr::V4(value) => {
            if prefix > 32 {
                return Err(DirectoryContractError::IpNetwork);
            }
            let raw = u32::from(value);
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32_u32 - u32::from(prefix))
            };
            Ok(IpAddr::V4(Ipv4Addr::from(raw & mask)))
        }
        IpAddr::V6(value) => {
            if prefix > 128 {
                return Err(DirectoryContractError::IpNetwork);
            }
            let raw = u128::from(value);
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128_u32 - u32::from(prefix))
            };
            Ok(IpAddr::V6(Ipv6Addr::from(raw & mask)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_workspace_role_name, AuditCursor, AuditListQuery, AuditListQueryDto,
        DirectoryContractError, DirectoryJoinOutcome, GuildIpBanByUserRequest,
        GuildIpBanByUserRequestDto, GuildIpBanId, GuildIpBanListQuery, GuildIpBanListQueryDto,
        IpNetwork, WorkspaceRoleId, AUDIT_ACCESS_DENIED_ERROR, DIRECTORY_JOIN_IP_BANNED_ERROR,
        DIRECTORY_JOIN_NOT_ALLOWED_ERROR, DIRECTORY_JOIN_USER_BANNED_ERROR, MAX_AUDIT_CURSOR_CHARS,
    };

    #[test]
    fn guild_ip_ban_id_requires_ulid() {
        let valid = GuildIpBanId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        assert!(valid.is_ok());
        let invalid = GuildIpBanId::try_from(String::from("ban-123"));
        assert_eq!(invalid, Err(DirectoryContractError::Id));
    }

    #[test]
    fn workspace_role_id_requires_ulid() {
        let valid = WorkspaceRoleId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        assert!(valid.is_ok());
        let invalid = WorkspaceRoleId::try_from(String::from("role-123"));
        assert_eq!(invalid, Err(DirectoryContractError::RoleId));
    }

    #[test]
    fn workspace_role_name_validation_enforces_bounds_and_charset() {
        assert_eq!(
            validate_workspace_role_name(" Moderator ").expect("valid role name"),
            "Moderator"
        );
        assert!(validate_workspace_role_name("").is_err());
        assert!(validate_workspace_role_name(&"a".repeat(64)).is_err());
        assert!(validate_workspace_role_name("bad\nname").is_err());
    }

    #[test]
    fn ip_network_canonicalizes_host_and_cidr() {
        let host = IpNetwork::try_from(String::from("203.0.113.15")).expect("host parses");
        assert_eq!(host.canonical_cidr(), "203.0.113.15/32");

        let cidr = IpNetwork::try_from(String::from("203.0.113.198/24")).expect("cidr parses");
        assert_eq!(cidr.canonical_cidr(), "203.0.113.0/24");
        assert!(cidr.contains("203.0.113.45".parse().expect("valid ip")));
        assert!(!cidr.contains("198.51.100.12".parse().expect("valid ip")));

        let ipv6 = IpNetwork::try_from(String::from("2001:DB8::F00D/64")).expect("ipv6 parses");
        assert_eq!(ipv6.canonical_cidr(), "2001:db8::/64");
    }

    #[test]
    fn ip_network_rejects_invalid_prefixes() {
        assert_eq!(
            IpNetwork::try_from(String::from("203.0.113.1/33")),
            Err(DirectoryContractError::IpNetwork)
        );
        assert_eq!(
            IpNetwork::try_from(String::from("2001:db8::1/129")),
            Err(DirectoryContractError::IpNetwork)
        );
    }

    #[test]
    fn audit_cursor_is_bounded_and_charset_limited() {
        let cursor = AuditCursor::try_from(String::from("abcDEF_123-xyz"));
        assert!(cursor.is_ok());
        assert_eq!(
            AuditCursor::try_from(String::from("cursor-with-space ")),
            Err(DirectoryContractError::AuditCursor)
        );
        assert_eq!(
            AuditCursor::try_from("a".repeat(MAX_AUDIT_CURSOR_CHARS + 1)),
            Err(DirectoryContractError::AuditCursor)
        );
    }

    #[test]
    fn audit_list_query_validation_enforces_limits_and_prefix_charset() {
        let query = AuditListQuery::try_from(AuditListQueryDto {
            cursor: Some(String::from("abc123")),
            limit: Some(25),
            action_prefix: Some(String::from("directory.join")),
        });
        assert!(query.is_ok());

        let invalid_limit = AuditListQuery::try_from(AuditListQueryDto {
            cursor: None,
            limit: Some(0),
            action_prefix: None,
        });
        assert_eq!(invalid_limit, Err(DirectoryContractError::Limit));

        let invalid_prefix = AuditListQuery::try_from(AuditListQueryDto {
            cursor: None,
            limit: Some(10),
            action_prefix: Some(String::from("Directory.Join")),
        });
        assert_eq!(invalid_prefix, Err(DirectoryContractError::ActionPrefix));
    }

    #[test]
    fn ip_ban_list_query_validation_enforces_limits() {
        let query = GuildIpBanListQuery::try_from(GuildIpBanListQueryDto {
            cursor: Some(String::from("cursor-1")),
            limit: Some(50),
        });
        assert!(query.is_ok());
        let invalid = GuildIpBanListQuery::try_from(GuildIpBanListQueryDto {
            cursor: None,
            limit: Some(101),
        });
        assert_eq!(invalid, Err(DirectoryContractError::Limit));
    }

    #[test]
    fn ip_ban_by_user_request_validation_enforces_domain_invariants() {
        let valid = GuildIpBanByUserRequest::try_from(GuildIpBanByUserRequestDto {
            target_user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            reason: Some(String::from("repeat raid joins")),
            expires_in_secs: Some(3_600),
        });
        assert!(valid.is_ok());

        let invalid_user = GuildIpBanByUserRequest::try_from(GuildIpBanByUserRequestDto {
            target_user_id: String::from("not-ulid"),
            reason: Some(String::from("reason")),
            expires_in_secs: None,
        });
        assert_eq!(invalid_user, Err(DirectoryContractError::UserId));

        let invalid_reason = GuildIpBanByUserRequest::try_from(GuildIpBanByUserRequestDto {
            target_user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            reason: Some(String::from("   ")),
            expires_in_secs: None,
        });
        assert_eq!(invalid_reason, Err(DirectoryContractError::Reason));
    }

    #[test]
    fn join_outcome_contract_matches_error_and_audit_semantics() {
        assert_eq!(
            DirectoryJoinOutcome::RejectedVisibility.rejection_error(),
            Some(DIRECTORY_JOIN_NOT_ALLOWED_ERROR)
        );
        assert_eq!(
            DirectoryJoinOutcome::RejectedUserBan.rejection_error(),
            Some(DIRECTORY_JOIN_USER_BANNED_ERROR)
        );
        assert_eq!(
            DirectoryJoinOutcome::RejectedIpBan.rejection_error(),
            Some(DIRECTORY_JOIN_IP_BANNED_ERROR)
        );
        assert_eq!(
            DirectoryJoinOutcome::RejectedVisibility.audit_action(),
            "directory.join.rejected.visibility"
        );
        assert_eq!(
            DirectoryJoinOutcome::RejectedUserBan.audit_action(),
            "directory.join.rejected.user_ban"
        );
        assert_eq!(
            DirectoryJoinOutcome::RejectedIpBan.audit_action(),
            "directory.join.rejected.ip_ban"
        );
        assert_eq!(AUDIT_ACCESS_DENIED_ERROR, "audit_access_denied");
    }
}
