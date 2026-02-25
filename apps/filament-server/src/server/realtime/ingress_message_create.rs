use crate::server::{
    auth::ClientIp,
    core::{AppState, AuthContext},
    domain::enforce_guild_ip_ban_for_request,
};

use super::{create_message_internal, ingress_command::GatewayMessageCreateCommand};

pub(crate) async fn execute_message_create_command(
    state: &AppState,
    auth: &AuthContext,
    client_ip: ClientIp,
    request: GatewayMessageCreateCommand,
) -> Result<(), &'static str> {
    if enforce_guild_ip_ban_for_request(
        state,
        request.guild_id.as_str(),
        auth.user_id,
        client_ip,
        "gateway.message_create",
    )
    .await
    .is_err()
    {
        return Err("ip_banned");
    }

    let attachment_ids = attachment_ids_or_empty(request.attachment_ids);
    if create_message_internal(
        state,
        auth,
        request.guild_id.as_str(),
        request.channel_id.as_str(),
        request.content,
        attachment_ids,
    )
    .await
    .is_err()
    {
        return Err("message_rejected");
    }

    Ok(())
}

pub(crate) fn attachment_ids_or_empty(attachment_ids: Option<Vec<String>>) -> Vec<String> {
    attachment_ids.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::attachment_ids_or_empty;

    #[test]
    fn attachment_ids_or_empty_returns_empty_for_none() {
        let ids = attachment_ids_or_empty(None);

        assert!(ids.is_empty());
    }

    #[test]
    fn attachment_ids_or_empty_returns_original_ids_for_some() {
        let ids = attachment_ids_or_empty(Some(vec![String::from("a1"), String::from("a2")]));

        assert_eq!(ids, vec![String::from("a1"), String::from("a2")]);
    }
}
