use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cli::{DeleteFileArgs, DownloadFileArgs, FilesCommand, UploadFileArgs};
use crate::client::ApiClient;
use crate::commands::prompt_confirm;
use crate::errors::CliError;

pub async fn handle_files_command(
    client: &ApiClient,
    command: &FilesCommand,
) -> Result<Value, CliError> {
    match command {
        FilesCommand::List(args) => Ok(client
            .list_files(
                args.run_id.as_deref(),
                args.connector_id.as_deref(),
                args.filename.as_deref(),
                args.source.as_deref(),
                args.sort.as_deref(),
                args.order.as_deref(),
                args.limit,
                args.cursor.as_deref(),
            )
            .await?),
        FilesCommand::Get(args) => Ok(client.get_file(&args.file_id).await?),
        FilesCommand::Upload(args) => upload_file(client, args).await,
        FilesCommand::Finalize(args) => Ok(client.finalize_file(&args.file_id).await?),
        FilesCommand::Delete(DeleteFileArgs { file_id, yes }) => {
            delete_file(client, file_id, *yes).await
        }
        FilesCommand::DownloadUrl(args) => Ok(client.get_file_download_url(&args.file_id).await?),
        FilesCommand::Download(args) => download_file(client, args).await,
    }
}

async fn upload_file(client: &ApiClient, args: &UploadFileArgs) -> Result<Value, CliError> {
    let bytes = std::fs::read(&args.path)?;
    let name = args
        .name
        .clone()
        .or_else(|| {
            args.path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| {
            CliError::Message("could not determine a file name for upload".to_string())
        })?;
    let content_type = args
        .content_type
        .clone()
        .unwrap_or_else(|| guess_content_type(&args.path).to_string());

    Ok(client.upload_file(&name, &content_type, bytes).await?)
}

async fn delete_file(client: &ApiClient, file_id: &str, yes: bool) -> Result<Value, CliError> {
    if !yes {
        let confirmed = prompt_confirm(&format!("Delete file `{file_id}`?"))?;
        if !confirmed {
            return Ok(json!({
                "deleted": false,
                "id": file_id,
                "message": "aborted"
            }));
        }
    }

    Ok(client.delete_file(file_id).await?)
}

async fn download_file(client: &ApiClient, args: &DownloadFileArgs) -> Result<Value, CliError> {
    let file = client.get_file(&args.file_id).await?;
    let default_name = file
        .get("name")
        .and_then(Value::as_str)
        .map(safe_filename)
        .unwrap_or_else(|| "download".to_string());
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(&default_name));

    confirm_overwrite(&output, args.yes)?;

    let downloaded = client.download_file_bytes(&args.file_id).await?;
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, &downloaded)?;

    Ok(json!({
        "file_id": args.file_id,
        "name": file.get("name").cloned().unwrap_or(Value::String(default_name)),
        "path": output.display().to_string(),
        "size_bytes": downloaded.len(),
        "content_type": file.get("content_type").cloned().unwrap_or(Value::Null),
    }))
}

fn confirm_overwrite(path: &Path, yes: bool) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }

    if yes {
        return Ok(());
    }

    if std::io::stdin().is_terminal() {
        let confirmed = prompt_confirm(&format!("Overwrite existing file `{}`?", path.display()))?;
        if confirmed {
            return Ok(());
        }
        return Err(CliError::Message("download aborted".to_string()));
    }

    Err(CliError::Message(format!(
        "file `{}` already exists; pass `--yes` to overwrite",
        path.display()
    )))
}

fn safe_filename(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .unwrap_or("download")
        .to_string()
}

fn guess_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("txt") | Some("log") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("xml") => "application/xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("md") => "text/markdown",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{guess_content_type, safe_filename};
    use std::path::Path;

    #[test]
    fn guesses_common_content_types() {
        assert_eq!(
            guess_content_type(Path::new("report.pdf")),
            "application/pdf"
        );
        assert_eq!(guess_content_type(Path::new("notes.TXT")), "text/plain");
        assert_eq!(
            guess_content_type(Path::new("noext")),
            "application/octet-stream"
        );
    }

    #[test]
    fn sanitizes_download_filenames() {
        assert_eq!(safe_filename("invoice.pdf"), "invoice.pdf");
        assert_eq!(safe_filename("/tmp/../secret"), "secret");
        assert_eq!(safe_filename(".."), "download");
    }
}
