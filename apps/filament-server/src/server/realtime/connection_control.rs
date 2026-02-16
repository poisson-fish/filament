use std::collections::HashMap;

use tokio::sync::watch;
use uuid::Uuid;

use crate::server::core::ConnectionControl;

pub(crate) fn signal_slow_connections_close(
    controls: &HashMap<Uuid, watch::Sender<ConnectionControl>>,
    slow_connections: Vec<Uuid>,
) {
    for connection_id in slow_connections {
        if let Some(control) = controls.get(&connection_id) {
            let _ = control.send(ConnectionControl::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::watch;
    use uuid::Uuid;

    use super::signal_slow_connections_close;
    use crate::server::core::ConnectionControl;

    #[test]
    fn closes_only_requested_connections_with_registered_controls() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let missing = Uuid::new_v4();

        let (first_tx, first_rx) = watch::channel(ConnectionControl::Open);
        let (second_tx, second_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(first, first_tx);
        controls.insert(second, second_tx);

        signal_slow_connections_close(&controls, vec![first, missing]);

        assert_eq!(*first_rx.borrow(), ConnectionControl::Close);
        assert_eq!(*second_rx.borrow(), ConnectionControl::Open);
    }
}