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

    if create_message_internal(
        state,
        auth,
        request.guild_id.as_str(),
        request.channel_id.as_str(),
        request.content,
        request.attachment_ids.into_vec(),
    )
    .await
    .is_err()
    {
        return Err("message_rejected");
    }

    Ok(())
}
