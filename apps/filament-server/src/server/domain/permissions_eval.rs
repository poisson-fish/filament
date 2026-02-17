use filament_core::{ChannelPermissionOverwrite, PermissionSet};

pub(crate) fn apply_channel_layers(
    base: PermissionSet,
    everyone: ChannelPermissionOverwrite,
    role_aggregate: ChannelPermissionOverwrite,
    member: ChannelPermissionOverwrite,
) -> PermissionSet {
    let mut bits = base.bits();
    let (everyone_allow, everyone_deny) =
        normalize_layer(everyone.allow.bits(), everyone.deny.bits());
    bits &= !everyone_deny;
    bits |= everyone_allow;

    let (role_allow, role_deny) =
        normalize_layer(role_aggregate.allow.bits(), role_aggregate.deny.bits());
    bits &= !role_deny;
    bits |= role_allow;

    let (member_allow, member_deny) = normalize_layer(member.allow.bits(), member.deny.bits());
    bits &= !member_deny;
    bits |= member_allow;
    PermissionSet::from_bits(bits)
}

fn normalize_layer(allow_bits: u64, deny_bits: u64) -> (u64, u64) {
    (allow_bits & !deny_bits, deny_bits)
}

#[cfg(test)]
mod tests {
    use super::apply_channel_layers;
    use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet};

    fn permission_set(values: &[Permission]) -> PermissionSet {
        let mut set = PermissionSet::empty();
        for value in values {
            set.insert(*value);
        }
        set
    }

    #[test]
    fn apply_channel_layers_follows_locked_precedence() {
        let base = permission_set(&[Permission::CreateMessage, Permission::DeleteMessage]);
        let everyone = ChannelPermissionOverwrite {
            allow: PermissionSet::empty(),
            deny: permission_set(&[Permission::CreateMessage]),
        };
        let roles = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::CreateMessage]),
            deny: permission_set(&[Permission::DeleteMessage]),
        };
        let member = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::DeleteMessage]),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let resolved = apply_channel_layers(base, everyone, roles, member);
        assert!(!resolved.contains(Permission::CreateMessage));
        assert!(resolved.contains(Permission::DeleteMessage));
    }

    #[test]
    fn apply_channel_layers_prefers_deny_when_same_layer_conflicts() {
        let base = permission_set(&[Permission::CreateMessage]);
        let member = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::CreateMessage]),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let resolved = apply_channel_layers(
            base,
            ChannelPermissionOverwrite::default(),
            ChannelPermissionOverwrite::default(),
            member,
        );
        assert!(!resolved.contains(Permission::CreateMessage));
    }
}
