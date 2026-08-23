use serde_json::{Value, json};

use crate::cli::{ConnectorsCommand, DeleteConnectorArgs, RenameConnectorArgs};
use crate::client::ApiClient;
use crate::commands::prompt_confirm;
use crate::errors::CliError;

pub async fn handle_connectors_command(
    client: &ApiClient,
    command: &ConnectorsCommand,
) -> Result<Value, CliError> {
    match command {
        ConnectorsCommand::List(args) => Ok(client
            .list_connectors(args.limit, args.cursor.as_deref(), args.domain.as_deref())
            .await?),
        ConnectorsCommand::Get(args) => Ok(client.get_connector(&args.connector_id).await?),
        ConnectorsCommand::Rename(RenameConnectorArgs {
            connector_id,
            display_name,
        }) => Ok(client.rename_connector(connector_id, display_name).await?),
        ConnectorsCommand::Delete(DeleteConnectorArgs { connector_id, yes }) => {
            delete_connector(client, connector_id, *yes).await
        }
        ConnectorsCommand::Revisions(args) => {
            Ok(client.list_connector_revisions(&args.connector_id).await?)
        }
    }
}

async fn delete_connector(
    client: &ApiClient,
    connector_id: &str,
    yes: bool,
) -> Result<Value, CliError> {
    if !yes {
        let confirmed = prompt_confirm(&format!("Delete connector `{connector_id}`?"))?;
        if !confirmed {
            return Ok(json!({
                "deleted": false,
                "id": connector_id,
                "message": "aborted"
            }));
        }
    }

    Ok(client.delete_connector(connector_id).await?)
}
