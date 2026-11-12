use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use flate2::read::GzDecoder;
use serde::Deserialize;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::config::StudioSourcemapConfig;
use crate::sourcemap::{self, Node};

#[derive(Clone, Debug)]
pub struct StudioContext {
    pub workspace: PathBuf,
    pub studio: StudioSourcemapConfig,
    pub assets_rel: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
}

#[derive(Deserialize)]
struct FullPayload {
    tree: Node,
}

pub fn serve(ctx: StudioContext, once: bool) -> Result<(), String> {
    let addr = format!("127.0.0.1:{}", ctx.studio.port);
    let server = Server::http(&addr).map_err(|e| format!("cannot listen on http://{addr}: {e}"))?;
    if !ctx.quiet {
        eprintln!("studio sourcemap listener: http://{addr} (POST /full, POST /clear)");
    }
    let ctx = Arc::new(Mutex::new(ctx));

    for request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let result = match (&method, url.as_str()) {
            (&Method::Get, "/get-file-paths") => respond_json(request, 200, "[]"),
            (&Method::Post, "/full") => handle_full(request, &ctx),
            (&Method::Post, "/clear") => handle_clear(request, &ctx),
            _ => {
                let _ = request.respond(Response::new_empty(StatusCode(404)));
                Ok(false)
            }
        };

        match result {
            Ok(done) => {
                if once && done {
                    break;
                }
            }
            Err(e) => {
                eprintln!("error: studio sourcemap: {e}");
            }
        }
    }
    Ok(())
}

pub fn spawn_background(ctx: StudioContext) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = serve(ctx, false) {
            eprintln!("error: studio sourcemap listener stopped: {e}");
        }
    })
}

fn handle_full(
    mut request: tiny_http::Request,
    ctx: &Arc<Mutex<StudioContext>>,
) -> Result<bool, String> {
    let content_encoding = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Content-Encoding"))
        .map(|h| h.value.as_str().to_ascii_lowercase());

    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|e| format!("cannot read /full body: {e}"))?;

    let verbose_flag = {
        let guard = ctx
            .lock()
            .map_err(|_| "studio context poisoned".to_string())?;
        guard.verbose
    };
    if verbose_flag {
        eprintln!("received studio /full payload: {} bytes", body.len());
    }

    let is_gzip = content_encoding
        .as_deref()
        .map(|v| v.contains("gzip"))
        .unwrap_or(false)
        || body.starts_with(&[0x1F, 0x8B]);
    if is_gzip {
        let mut decoded = Vec::new();
        let mut decoder = GzDecoder::new(body.as_slice());
        decoder
            .read_to_end(&mut decoded)
            .map_err(|e| format!("cannot decompress /full gzip body: {e}"))?;
        body = decoded;
        if verbose_flag {
            eprintln!("decompressed studio /full payload: {} bytes", body.len());
        }
    }

    // Strip UTF-8 BOM if present.
    if body.starts_with(&[0xEF, 0xBB, 0xBF]) {
        body.drain(0..3);
    }
    let trimmed = trim_ascii(&body);
    if trimmed.is_empty() {
        return Err("invalid /full payload: empty body".to_string());
    }

    let tree: Node = if let Ok(payload) = serde_json::from_slice::<FullPayload>(trimmed) {
        payload.tree
    } else if let Ok(node) = serde_json::from_slice::<Node>(trimmed) {
        node
    } else if let Ok(value) = serde_json::from_slice::<serde_json::Value>(trimmed) {
        if let Some(t) = value.get("tree") {
            serde_json::from_value(t.clone())
                .map_err(|e| format!("invalid /full payload tree: {e}"))?
        } else {
            return Err("invalid /full payload: missing `tree`".to_string());
        }
    } else {
        let preview =
            String::from_utf8_lossy(&trimmed[..trimmed.len().min(120)]).replace('\n', "\\n");
        return Err(format!(
            "invalid /full payload: expected JSON object, got {} bytes: {:?}",
            trimmed.len(),
            preview
        ));
    };

    let (out, verbose, quiet) = {
        let guard = ctx
            .lock()
            .map_err(|_| "studio context poisoned".to_string())?;
        let out = sourcemap::apply_studio_tree(
            tree,
            &guard.workspace,
            &guard.studio,
            &guard.assets_rel,
            guard.verbose,
        )?;
        (out, guard.verbose, guard.quiet)
    };

    if !quiet {
        match out {
            Some(p) => eprintln!("updated studio sourcemap {}", p.display()),
            None => eprintln!("updated studio sourcemap (no matching nodes)"),
        }
    } else if verbose {
        eprintln!("studio sourcemap updated");
    }

    let _ = request.respond(
        Response::from_string("{\"ok\":true}")
            .with_status_code(StatusCode(200))
            .with_header(json_header()),
    );
    Ok(true)
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn handle_clear(
    request: tiny_http::Request,
    ctx: &Arc<Mutex<StudioContext>>,
) -> Result<bool, String> {
    {
        let guard = ctx
            .lock()
            .map_err(|_| "studio context poisoned".to_string())?;
        sourcemap::clear_studio_sourcemap(
            &guard.workspace,
            &guard.studio,
            &guard.assets_rel,
            guard.verbose,
        )?;
        if !guard.quiet {
            eprintln!("cleared studio sourcemap");
        }
    }
    let _ = request.respond(
        Response::from_string("{\"ok\":true}")
            .with_status_code(StatusCode(200))
            .with_header(json_header()),
    );
    Ok(true)
}

fn respond_json(request: tiny_http::Request, status: u16, body: &str) -> Result<bool, String> {
    let _ = request.respond(
        Response::from_string(body.to_string())
            .with_status_code(StatusCode(status))
            .with_header(json_header()),
    );
    Ok(false)
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}
