//! Async license-text fetching from the upstream repository.

use gpui::Context;
use gpui::http_client::{AsyncBody, HttpClient as _, http};
use futures::AsyncReadExt;
use reqwest_client::ReqwestClient;
use super::super::InstallerView;

const LICENSE_URL: &str =
    "https://raw.githubusercontent.com/Far-Beyond-Pulsar/Pulsar-Native/main/LICENSE.md";

impl InstallerView {
    /// Kick off a background fetch of the project license text.
    /// Updates `license_text` / `license_fetch_error` and notifies on completion.
    pub fn fetch_license(&mut self, cx: &mut Context<Self>) {
        self.loading_license = true;
        self.license_fetch_error = None;
        self.license_text = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            // Build HTTP client.
            let client = match ReqwestClient::user_agent("Pulsar-Installer/1.0") {
                Ok(c) => c,
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("Client error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            // Build request.
            let request = match http::Request::builder()
                .method("GET")
                .uri(LICENSE_URL)
                .body(AsyncBody::default())
            {
                Ok(r) => r,
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("Request build error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            // Send and read.
            match client.send(request).await {
                Ok(mut response) => {
                    let mut body = Vec::new();
                    if response.body_mut().read_to_end(&mut body).await.is_ok() {
                        let text = String::from_utf8_lossy(&body).into_owned();
                        this.update(cx, |v, cx| {
                            v.license_text = Some(text);
                            v.loading_license = false;
                            cx.notify();
                        })
                        .ok();
                    } else {
                        this.update(cx, |v, cx| {
                            v.license_fetch_error =
                                Some("Failed to read response body".to_string());
                            v.loading_license = false;
                            cx.notify();
                        })
                        .ok();
                    }
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("HTTP error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}
