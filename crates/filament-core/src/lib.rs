#![forbid(unsafe_code)]

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarkdownToken {
    ParagraphStart,
    ParagraphEnd,
    EmphasisStart,
    EmphasisEnd,
    StrongStart,
    StrongEnd,
    ListStart { ordered: bool },
    ListEnd,
    ListItemStart,
    ListItemEnd,
    LinkStart { href: String },
    LinkEnd,
    Text { text: String },
    Code { code: String },
    SoftBreak,
    HardBreak,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Moderator,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ManageRoles,
    ManageChannelOverrides,
    DeleteMessage,
    BanMember,
    CreateMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PermissionSet(u64);

impl PermissionSet {
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn contains(self, permission: Permission) -> bool {
        self.0 & permission_mask(permission) != 0
    }

    pub fn insert(&mut self, permission: Permission) {
        self.0 |= permission_mask(permission);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChannelPermissionOverwrite {
    pub allow: PermissionSet,
    pub deny: PermissionSet,
}

#[must_use]
pub fn role_rank(role: Role) -> u8 {
    match role {
        Role::Owner => 3,
        Role::Moderator => 2,
        Role::Member => 1,
    }
}

#[must_use]
pub fn can_assign_role(actor: Role, current_target: Role, new_target: Role) -> bool {
    if matches!(current_target, Role::Owner) || matches!(new_target, Role::Owner) {
        return false;
    }
    if !has_permission(actor, Permission::ManageRoles) {
        return false;
    }
    role_rank(actor) > role_rank(current_target) && role_rank(actor) > role_rank(new_target)
}

#[must_use]
pub fn can_moderate_member(actor: Role, target: Role) -> bool {
    if !has_permission(actor, Permission::BanMember) {
        return false;
    }
    !matches!(target, Role::Owner) && role_rank(actor) > role_rank(target)
}

#[must_use]
pub fn base_permissions(role: Role) -> PermissionSet {
    let mut set = PermissionSet::empty();
    match role {
        Role::Owner => {
            set.insert(Permission::ManageRoles);
            set.insert(Permission::ManageChannelOverrides);
            set.insert(Permission::DeleteMessage);
            set.insert(Permission::BanMember);
            set.insert(Permission::CreateMessage);
        }
        Role::Moderator => {
            set.insert(Permission::DeleteMessage);
            set.insert(Permission::BanMember);
            set.insert(Permission::CreateMessage);
        }
        Role::Member => {
            set.insert(Permission::CreateMessage);
        }
    }
    set
}

#[must_use]
pub fn apply_channel_overwrite(
    base: PermissionSet,
    overwrite: Option<ChannelPermissionOverwrite>,
) -> PermissionSet {
    let Some(overwrite) = overwrite else {
        return base;
    };
    let mut bits = base.bits();
    bits |= overwrite.allow.bits();
    bits &= !overwrite.deny.bits();
    PermissionSet::from_bits(bits)
}

fn permission_mask(permission: Permission) -> u64 {
    match permission {
        Permission::ManageRoles => 1 << 0,
        Permission::ManageChannelOverrides => 1 << 1,
        Permission::DeleteMessage => 1 << 2,
        Permission::BanMember => 1 << 3,
        Permission::CreateMessage => 1 << 4,
    }
}

#[must_use]
pub fn has_permission(role: Role, permission: Permission) -> bool {
    base_permissions(role).contains(permission)
}

#[must_use]
pub fn tokenize_markdown(markdown: &str) -> Vec<MarkdownToken> {
    let mut tokens = Vec::new();
    let parser = Parser::new_ext(markdown, Options::empty());
    let mut link_stack: Vec<bool> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => tokens.push(MarkdownToken::ParagraphStart),
                Tag::Emphasis => tokens.push(MarkdownToken::EmphasisStart),
                Tag::Strong => tokens.push(MarkdownToken::StrongStart),
                Tag::List(start) => tokens.push(MarkdownToken::ListStart {
                    ordered: start.is_some(),
                }),
                Tag::Item => tokens.push(MarkdownToken::ListItemStart),
                Tag::Link { dest_url, .. } => {
                    if let Some(href) = sanitize_link_target(dest_url.as_ref()) {
                        tokens.push(MarkdownToken::LinkStart { href });
                        link_stack.push(true);
                    } else {
                        link_stack.push(false);
                    }
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => tokens.push(MarkdownToken::ParagraphEnd),
                TagEnd::Emphasis => tokens.push(MarkdownToken::EmphasisEnd),
                TagEnd::Strong => tokens.push(MarkdownToken::StrongEnd),
                TagEnd::List(_) => tokens.push(MarkdownToken::ListEnd),
                TagEnd::Item => tokens.push(MarkdownToken::ListItemEnd),
                TagEnd::Link => {
                    if link_stack.pop().unwrap_or(false) {
                        tokens.push(MarkdownToken::LinkEnd);
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if !text.is_empty() {
                    tokens.push(MarkdownToken::Text {
                        text: text.into_string(),
                    });
                }
            }
            Event::Code(code) => tokens.push(MarkdownToken::Code {
                code: code.into_string(),
            }),
            Event::SoftBreak => tokens.push(MarkdownToken::SoftBreak),
            Event::HardBreak => tokens.push(MarkdownToken::HardBreak),
            _ => {}
        }
    }

    tokens
}

fn sanitize_link_target(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (scheme, _) = trimmed.split_once(':')?;
    let scheme = scheme.to_ascii_lowercase();
    if matches!(scheme.as_str(), "http" | "https" | "mailto") {
        Some(trimmed.to_owned())
    } else {
        None
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
        apply_channel_overwrite, base_permissions, can_assign_role, can_moderate_member,
        has_permission, project_name, role_rank, tokenize_markdown, ChannelName,
        ChannelPermissionOverwrite, DomainError, GuildName, MarkdownToken, Permission,
        PermissionSet, Role, UserId, Username,
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
        assert!(has_permission(Role::Owner, Permission::ManageRoles));
        assert!(has_permission(Role::Moderator, Permission::DeleteMessage));
        assert!(!has_permission(Role::Moderator, Permission::ManageRoles));
        assert!(!has_permission(Role::Member, Permission::DeleteMessage));
        assert!(has_permission(Role::Member, Permission::CreateMessage));
    }

    #[test]
    fn role_hierarchy_and_assignment_rules_are_enforced() {
        assert!(role_rank(Role::Owner) > role_rank(Role::Moderator));
        assert!(role_rank(Role::Moderator) > role_rank(Role::Member));
        assert!(can_assign_role(Role::Owner, Role::Member, Role::Moderator));
        assert!(can_assign_role(Role::Owner, Role::Moderator, Role::Member));
        assert!(!can_assign_role(
            Role::Moderator,
            Role::Member,
            Role::Moderator
        ));
        assert!(!can_assign_role(Role::Owner, Role::Owner, Role::Member));
        assert!(!can_assign_role(Role::Owner, Role::Member, Role::Owner));
        assert!(can_moderate_member(Role::Owner, Role::Moderator));
        assert!(can_moderate_member(Role::Moderator, Role::Member));
        assert!(!can_moderate_member(Role::Moderator, Role::Moderator));
        assert!(!can_moderate_member(Role::Moderator, Role::Owner));
    }

    #[test]
    fn channel_overrides_apply_allow_and_deny_masks() {
        let base = base_permissions(Role::Member);
        assert!(base.contains(Permission::CreateMessage));
        assert!(!base.contains(Permission::DeleteMessage));

        let overwrite = ChannelPermissionOverwrite {
            allow: PermissionSet::from_bits(1 << 2),
            deny: PermissionSet::from_bits(1 << 4),
        };
        let effective = apply_channel_overwrite(base, Some(overwrite));
        assert!(effective.contains(Permission::DeleteMessage));
        assert!(!effective.contains(Permission::CreateMessage));
    }

    #[test]
    fn user_id_round_trip_and_parse_validation() {
        let id = UserId::new();
        let parsed = UserId::try_from(id.to_string()).unwrap();
        assert_eq!(id, parsed);

        let invalid = UserId::try_from(String::from("not-a-ulid")).unwrap_err();
        assert_eq!(invalid, DomainError::InvalidUserId);
    }

    #[test]
    fn markdown_strips_html_and_rejects_unsafe_links() {
        let html_tokens = tokenize_markdown("<script>alert(1)</script>");
        assert!(html_tokens.is_empty());

        let tokens = tokenize_markdown("[x](javascript:alert(1)) [ok](https://example.com)");
        assert!(!tokens.iter().any(|token| matches!(
            token,
            MarkdownToken::LinkStart { href } if href.starts_with("javascript:")
        )));
        assert!(tokens
            .iter()
            .any(|token| matches!(token, MarkdownToken::LinkStart { href } if href.starts_with("https://example.com"))));
    }

    #[test]
    fn markdown_allowlist_keeps_basic_formatting_tokens() {
        let tokens = tokenize_markdown("**hi** _there_ `x`");
        assert!(tokens.contains(&MarkdownToken::StrongStart));
        assert!(tokens.contains(&MarkdownToken::StrongEnd));
        assert!(tokens.contains(&MarkdownToken::EmphasisStart));
        assert!(tokens.contains(&MarkdownToken::EmphasisEnd));
        assert!(tokens.contains(&MarkdownToken::Code {
            code: String::from("x"),
        }));
    }
}
