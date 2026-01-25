//! Local HTTP callback server for OAuth redirects
//!
//! Starts a temporary HTTP server to receive OAuth callbacks from the browser.

use std::collections::HashMap;
use std::io::Cursor;
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use tiny_http::{Response, Server};

use super::OAuthProvider;

/// Result from the callback server
#[derive(Debug, Clone)]
pub struct CallbackResult {
    /// The authorization code from the OAuth provider
    pub code: String,
    /// The state parameter (for CSRF verification)
    pub state: String,
}

/// Callback server error
#[derive(Debug)]
pub enum CallbackError {
    /// Port is already in use
    PortInUse(u16),
    /// Server failed to start
    ServerError(String),
    /// Timeout waiting for callback
    Timeout,
    /// User cancelled or denied consent
    UserCancelled(String),
    /// Invalid callback parameters
    InvalidCallback(String),
}

impl std::fmt::Display for CallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallbackError::PortInUse(port) => {
                write!(f, "Port {} is already in use", port)
            }
            CallbackError::ServerError(msg) => {
                write!(f, "Server error: {}", msg)
            }
            CallbackError::Timeout => {
                write!(f, "Timeout waiting for OAuth callback")
            }
            CallbackError::UserCancelled(msg) => {
                write!(f, "Authentication cancelled: {}", msg)
            }
            CallbackError::InvalidCallback(msg) => {
                write!(f, "Invalid callback: {}", msg)
            }
        }
    }
}

impl std::error::Error for CallbackError {}

/// Check if a port is available
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok()
}

/// Start the callback server and wait for the OAuth redirect
///
/// Returns the authorization code and state from the callback.
/// Times out after the specified duration.
pub fn wait_for_callback(
    provider: OAuthProvider,
    expected_state: &str,
    timeout: Duration,
) -> Result<CallbackResult, CallbackError> {
    let port = provider.callback_port();
    let path = provider.callback_path();

    // Check if port is available
    if !is_port_available(port) {
        return Err(CallbackError::PortInUse(port));
    }

    // Create channel for communication between server thread and main thread
    let (tx, rx): (Sender<Result<CallbackResult, CallbackError>>, Receiver<_>) = mpsc::channel();

    let expected_state = expected_state.to_string();
    let expected_path = path.to_string();

    // Start server in a separate thread
    let server_thread = thread::spawn(move || {
        let addr = format!("127.0.0.1:{}", port);
        let server = match Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(CallbackError::ServerError(e.to_string())));
                return;
            }
        };

        log::info!("OAuth callback server listening on {}", addr);

        // Wait for a single request with timeout
        match server.recv_timeout(timeout) {
            Ok(Some(request)) => {
                let url = request.url().to_string();
                log::info!("OAuth callback server received request: {}", url);

                // Parse the callback
                let result = parse_callback(&url, &expected_path, &expected_state);
                log::info!("OAuth callback parse result: {:?}", result.is_ok());

                // Send response to browser
                let (status, body) = match &result {
                    Ok(_) => (200, success_page()),
                    Err(e) => (400, error_page(&e.to_string())),
                };

                let body_len = body.len();
                let response = Response::new(
                    tiny_http::StatusCode(status),
                    vec![tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap()],
                    Cursor::new(body),
                    Some(body_len),
                    None,
                );

                let _ = request.respond(response);
                log::info!("OAuth callback server sent response to browser");

                let send_result = tx.send(result);
                log::info!(
                    "OAuth callback server sent result through channel: {:?}",
                    send_result.is_ok()
                );
            }
            Ok(None) => {
                // Timeout
                let _ = tx.send(Err(CallbackError::Timeout));
            }
            Err(e) => {
                let _ = tx.send(Err(CallbackError::ServerError(e.to_string())));
            }
        }
    });

    // Wait for result from server thread
    log::info!("OAuth callback: waiting for result from server thread");
    let result = rx.recv().map_err(|_| CallbackError::Timeout)?;
    log::info!("OAuth callback: received result from server thread");

    // Clean up server thread
    let _ = server_thread.join();
    log::info!("OAuth callback: server thread joined");

    result
}

/// Parse the callback URL and extract code and state
fn parse_callback(
    url: &str,
    expected_path: &str,
    expected_state: &str,
) -> Result<CallbackResult, CallbackError> {
    // Check path
    let path = url.split('?').next().unwrap_or("");
    if path != expected_path {
        return Err(CallbackError::InvalidCallback(format!(
            "Unexpected path: {}",
            path
        )));
    }

    // Parse query parameters
    let query = url.split('?').nth(1).unwrap_or("");
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((
                urlencoding::decode(key).ok()?.into_owned(),
                urlencoding::decode(value).ok()?.into_owned(),
            ))
        })
        .collect();

    // Check for error response
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| error.clone());
        return Err(CallbackError::UserCancelled(description));
    }

    // Extract code
    let code = params
        .get("code")
        .ok_or_else(|| CallbackError::InvalidCallback("Missing 'code' parameter".to_string()))?
        .clone();

    // Extract and verify state
    let state = params
        .get("state")
        .ok_or_else(|| CallbackError::InvalidCallback("Missing 'state' parameter".to_string()))?
        .clone();

    if state != expected_state {
        return Err(CallbackError::InvalidCallback(
            "State mismatch - possible CSRF attack".to_string(),
        ));
    }

    Ok(CallbackResult { code, state })
}

/// Generate the success HTML page shown to the user after successful authentication
fn success_page() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Authentication Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .container {
            text-align: center;
            padding: 40px;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
        }
        .checkmark {
            font-size: 64px;
            margin-bottom: 20px;
        }
        h1 { margin: 0 0 10px 0; font-weight: 600; }
        p { opacity: 0.9; margin: 0; }
    </style>
</head>
<body>
    <div class="container">
        <div class="checkmark">✓</div>
        <h1>Authentication Successful</h1>
        <p>You can close this window and return to Ramble.</p>
    </div>
    <script>setTimeout(() => window.close(), 3000);</script>
</body>
</html>"#
        .to_string()
}

/// Generate the error HTML page shown when authentication fails
fn error_page(error: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Authentication Failed</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #e74c3c 0%, #c0392b 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 40px;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
            max-width: 500px;
        }}
        .icon {{ font-size: 64px; margin-bottom: 20px; }}
        h1 {{ margin: 0 0 10px 0; font-weight: 600; }}
        p {{ opacity: 0.9; margin: 10px 0; }}
        .error {{ font-family: monospace; font-size: 14px; opacity: 0.8; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">✗</div>
        <h1>Authentication Failed</h1>
        <p>Something went wrong during authentication.</p>
        <p class="error">{}</p>
        <p>Please close this window and try again in Ramble.</p>
    </div>
</body>
</html>"#,
        html_escape(error)
    )
}

/// Basic HTML escaping for error messages
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_callback_success() {
        let url = "/oauth2callback?code=abc123&state=xyz789";
        let result = parse_callback(url, "/oauth2callback", "xyz789").unwrap();
        assert_eq!(result.code, "abc123");
        assert_eq!(result.state, "xyz789");
    }

    #[test]
    fn test_parse_callback_state_mismatch() {
        let url = "/oauth2callback?code=abc123&state=wrong";
        let result = parse_callback(url, "/oauth2callback", "xyz789");
        assert!(matches!(result, Err(CallbackError::InvalidCallback(_))));
    }

    #[test]
    fn test_parse_callback_error_response() {
        let url = "/oauth2callback?error=access_denied&error_description=User%20denied%20access";
        let result = parse_callback(url, "/oauth2callback", "xyz789");
        assert!(matches!(result, Err(CallbackError::UserCancelled(_))));
    }
}
