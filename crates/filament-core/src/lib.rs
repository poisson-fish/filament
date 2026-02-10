#![forbid(unsafe_code)]

use ulid::Ulid;

/// Returns the project code name.
#[must_use]
pub const fn project_name() -> &'static str {
    "filament"
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("name is invalid")]
    InvalidName,
    #[error("username is invalid")]
    InvalidUsername,
    #[error("user id is invalid")]
    InvalidUserId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(Ulid);

impl UserId {
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for UserId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| DomainError::InvalidUserId)?;
        Ok(Self(parsed))
    }
}

impl core::fmt::Display for UserId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Username {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_username(&value)?;
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuildName(String);

impl GuildName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GuildName {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_name(&value, 1, 64)?;
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelName(String);

impl ChannelName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChannelName {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_name(&value, 1, 64)?;
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Moderator,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    DeleteMessage,
    BanMember,
    CreateMessage,
}

#[must_use]
pub fn has_permission(role: Role, permission: Permission) -> bool {
    match role {
        Role::Owner => true,
        Role::Moderator => matches!(
            permission,
            Permission::DeleteMessage | Permission::BanMember | Permission::CreateMessage
        ),
        Role::Member => matches!(permission, Permission::CreateMessage),
    }
}

fn validate_username(value: &str) -> Result<(), DomainError> {
    if !(3..=32).contains(&value.len()) {
        return Err(DomainError::InvalidUsername);
    }

    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Ok(());
    }

    Err(DomainError::InvalidUsername)
}

fn validate_name(value: &str, min: usize, max: usize) -> Result<(), DomainError> {
    if !(min..=max).contains(&value.len()) {
        return Err(DomainError::InvalidName);
    }

    if value.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        return Ok(());
    }

    Err(DomainError::InvalidName)
}

#[cfg(test)]
mod tests {
    use super::{
        has_permission, project_name, ChannelName, DomainError, GuildName, Permission, Role,
        UserId, Username,
    };

    #[test]
    fn project_name_is_stable() {
        assert_eq!(project_name(), "filament");
    }

    #[test]
    fn username_invariants_enforced() {
        let valid = Username::try_from(String::from("alice_1")).unwrap();
        assert_eq!(valid.as_str(), "alice_1");
        assert_eq!(
            Username::try_from(String::from("a")).unwrap_err(),
            DomainError::InvalidUsername
        );
        assert_eq!(
            Username::try_from(String::from("bad-name")).unwrap_err(),
            DomainError::InvalidUsername
        );
    }

    #[test]
    fn guild_and_channel_names_enforce_bounds() {
        let guild = GuildName::try_from(String::from("General Guild")).unwrap();
        let channel = ChannelName::try_from(String::from("general-chat")).unwrap();
        assert_eq!(guild.as_str(), "General Guild");
        assert_eq!(channel.as_str(), "general-chat");
    }

    #[test]
    fn permission_checks_match_role_expectations() {
        assert!(has_permission(Role::Owner, Permission::BanMember));
        assert!(has_permission(Role::Moderator, Permission::DeleteMessage));
        assert!(!has_permission(Role::Member, Permission::DeleteMessage));
        assert!(has_permission(Role::Member, Permission::CreateMessage));
    }

    #[test]
    fn user_id_round_trip_and_parse_validation() {
        let id = UserId::new();
        let parsed = UserId::try_from(id.to_string()).unwrap();
        assert_eq!(id, parsed);

        let invalid = UserId::try_from(String::from("not-a-ulid")).unwrap_err();
        assert_eq!(invalid, DomainError::InvalidUserId);
    }
}
