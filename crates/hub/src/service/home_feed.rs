use std::sync::Arc;

use gpui::RenderImage;

#[derive(Clone)]
pub struct HomeSlide {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub date: String,
    pub url: String,
    pub image: Option<Arc<RenderImage>>,
}

/// Fetch the public blog previews and current engine releases using the
/// launcher's bundled TLS backend, then decode every available cover before
/// returning so slide changes never block the UI thread.
pub fn load_home_feed_blocking() -> Vec<HomeSlide> {
    let client =
        match reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
            .user_agent("Pulsar-Hub/1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!("Could not create home feed client: {error}");
                return Vec::new();
            }
        };

    let mut slides = Vec::new();
    match client
        .get("https://pulsarnative.com/blog/blog-preview.json")
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Vec<serde_json::Value>>())
    {
        Ok(posts) => {
            for post in posts
                .into_iter()
                .filter(|post| {
                    !post
                        .get("draft")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
                })
                .take(5)
            {
                let title = json_text(&post, "title");
                let url = json_text(&post, "url");
                if title.is_empty() || url.is_empty() {
                    continue;
                }
                let image_url = json_text(&post, "thumbnail");
                slides.push(HomeSlide {
                    kind: "Pulsar Blog".into(),
                    title,
                    description: json_text(&post, "description"),
                    date: json_text(&post, "date"),
                    url,
                    image: fetch_image(&client, &image_url),
                });
            }
        }
        Err(error) => tracing::warn!("Could not load Pulsar blog previews: {error}"),
    }

    match crate::service::installer_service::fetch_repo_releases_blocking(
        "Far-Beyond-Pulsar/Pulsar-Native",
        1,
    ) {
        Ok(releases) => {
            for release in releases.into_iter().take(3) {
                let title = if release.name.trim().is_empty() {
                    release.tag_name.clone()
                } else {
                    release.name
                };
                slides.push(HomeSlide {
                    kind: "Pulsar Engine Release".into(),
                    title,
                    description: strip_markdown(&release.body),
                    date: release.published_at.chars().take(10).collect(),
                    url: format!(
                        "https://github.com/Far-Beyond-Pulsar/Pulsar-Native/releases/tag/{}",
                        release.tag_name
                    ),
                    image: None,
                });
            }
        }
        Err(error) => tracing::warn!("Could not load engine releases for home: {error}"),
    }
    slides
}

fn json_text(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn fetch_image(client: &reqwest::blocking::Client, url: &str) -> Option<Arc<RenderImage>> {
    if url.is_empty() {
        return None;
    }
    let bytes = client
        .get(url)
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let frame = image::Frame::new(rgba);
    Some(Arc::new(RenderImage::new(smallvec::smallvec![frame])))
}

fn strip_markdown(body: &str) -> String {
    let cleaned = body
        .lines()
        .map(|line| line.trim_start_matches('#').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    cleaned.chars().take(360).collect()
}
