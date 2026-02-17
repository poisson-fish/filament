use std::collections::HashMap;

use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::server::core::{ConnectionControl, ConnectionPresence};

pub(crate) fn remove_connection_state(
    presence: &mut HashMap<Uuid, ConnectionPresence>,
    controls: &mut HashMap<Uuid, watch::Sender<ConnectionControl>>,
    senders: &mut HashMap<Uuid, mpsc::Sender<String>>,
    connection_id: Uuid,
) -> Option<ConnectionPresence> {
    let removed_presence = presence.remove(&connection_id);
    controls.remove(&connection_id);
    senders.remove(&connection_id);
    removed_presence
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use tokio::sync::{mpsc, watch};
    use uuid::Uuid;

    use super::remove_connection_state;
    use crate::server::core::{ConnectionControl, ConnectionPresence};

    #[test]
    fn removes_presence_controls_and_sender_for_connection() {
        let connection_id = Uuid::new_v4();
        let user_id = UserId::new();
        let mut presence = HashMap::new();
        presence.insert(
            connection_id,
            ConnectionPresence {
                user_id,
                guild_ids: HashSet::new(),
            },
        );
        let (control_tx, _control_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(connection_id, control_tx);
        let (sender_tx, _sender_rx) = mpsc::channel::<String>(1);
        let mut senders = HashMap::new();
        senders.insert(connection_id, sender_tx);

        let removed =
            remove_connection_state(&mut presence, &mut controls, &mut senders, connection_id);

        assert_eq!(
            removed.expect("presence should be removed").user_id,
            user_id
        );
        assert!(!presence.contains_key(&connection_id));
        assert!(!controls.contains_key(&connection_id));
        assert!(!senders.contains_key(&connection_id));
    }

    #[test]
    fn returns_none_when_presence_is_missing_but_still_prunes_other_maps() {
        let connection_id = Uuid::new_v4();
        let (control_tx, _control_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(connection_id, control_tx);
        let (sender_tx, _sender_rx) = mpsc::channel::<String>(1);
        let mut senders = HashMap::new();
        senders.insert(connection_id, sender_tx);

        let removed = remove_connection_state(
            &mut HashMap::new(),
            &mut controls,
            &mut senders,
            connection_id,
        );

        assert!(removed.is_none());
        assert!(!controls.contains_key(&connection_id));
        assert!(!senders.contains_key(&connection_id));
    }
}
