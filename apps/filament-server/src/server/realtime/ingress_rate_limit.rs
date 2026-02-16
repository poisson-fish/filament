use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub(crate) fn allow_gateway_ingress(
    ingress: &mut VecDeque<Instant>,
    limit: u32,
    window: Duration,
) -> bool {
    let now = Instant::now();
    while ingress
        .front()
        .is_some_and(|oldest| now.duration_since(*oldest) > window)
    {
        let _ = ingress.pop_front();
    }

    if ingress.len() >= limit as usize {
        return false;
    }

    ingress.push_back(now);
    true
}

#[cfg(test)]
mod tests {
    use super::allow_gateway_ingress;
    use std::{
        collections::VecDeque,
        time::{Duration, Instant},
    };

    #[test]
    fn allows_when_under_limit() {
        let mut ingress = VecDeque::new();
        assert!(allow_gateway_ingress(
            &mut ingress,
            2,
            Duration::from_millis(250),
        ));
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn rejects_when_at_limit_inside_window() {
        let mut ingress = VecDeque::new();
        let now = Instant::now();
        ingress.push_back(now - Duration::from_millis(50));
        ingress.push_back(now - Duration::from_millis(10));

        assert!(!allow_gateway_ingress(
            &mut ingress,
            2,
            Duration::from_millis(250),
        ));
    }

    #[test]
    fn evicts_expired_entries_before_checking_limit() {
        let mut ingress = VecDeque::new();
        let now = Instant::now();
        ingress.push_back(now - Duration::from_secs(2));

        assert!(allow_gateway_ingress(
            &mut ingress,
            1,
            Duration::from_millis(100),
        ));
        assert_eq!(ingress.len(), 1);
    }
}
