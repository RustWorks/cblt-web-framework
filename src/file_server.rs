use crate::response::{error_response, send_response, send_response_file};
use http::{Request, Response, StatusCode};
use std::path::{Component, Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::instrument;

#[cfg_attr(debug_assertions, instrument(level = "trace", skip_all))]
pub async fn directive<S>(
    root_path: &Option<String>,
    request: &Request<Vec<u8>>,
    handled: &mut bool,
    socket: &mut S,
    req_opt: Option<&Request<Vec<u8>>>,
) where
    S: AsyncWriteExt + Unpin,
{
    match root_path {
        None => {
            let response = error_response(StatusCode::INTERNAL_SERVER_ERROR);
            let _ = send_response(&mut *socket, response, req_opt).await;
            *handled = true;
            return;
        }
        Some(root) => {
            if let Some(mut file_path) = sanitize_path(
                &Path::new(root),
                request.uri().path().trim_start_matches('/'),
            ) {
                if file_path.is_dir() {
                    file_path.push("index.html");
                }

                match File::open(&file_path).await {
                    Ok(file) => {
                        let content_length = file_size(&file).await;
                        let response = file_response(file, content_length);
                        let _ = send_response_file(socket, response, req_opt).await;
                        *handled = true;
                        return;
                    }
                    Err(_) => {
                        let response = error_response(StatusCode::NOT_FOUND);
                        let _ = send_response(&mut *socket, response, req_opt).await;
                        *handled = true;
                        return;
                    }
                }
            } else {
                let response = error_response(StatusCode::FORBIDDEN);
                let _ = send_response(&mut *socket, response, req_opt).await;
                *handled = true;
                return;
            }
        }
    }
}

#[cfg_attr(debug_assertions, instrument(level = "trace", skip_all))]
async fn file_size(file: &File) -> u64 {
    let metadata = file.metadata().await.unwrap();
    metadata.len()
}

#[cfg_attr(debug_assertions, instrument(level = "trace", skip_all))]
fn file_response(file: File, content_length: u64) -> Response<File> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Length", content_length)
        .body(file)
        .unwrap()
}

#[cfg_attr(debug_assertions, instrument(level = "trace", skip_all))]
fn sanitize_path(base_path: &Path, requested_path: &str) -> Option<PathBuf> {
    let mut full_path = base_path.to_path_buf();
    let requested_path = Path::new(requested_path);

    for component in requested_path.components() {
        match component {
            Component::Normal(segment) => full_path.push(segment),
            Component::RootDir | Component::Prefix(_) => return None,
            Component::ParentDir => {
                if !full_path.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
        }
    }

    if full_path.starts_with(base_path) {
        Some(full_path)
    } else {
        None
    }
}
