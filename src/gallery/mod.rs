use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use rand::Rng;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::{fs as fs, io::AsyncWriteExt};

use crate::ctx;

mod format;
pub use format::*;

impl ImageType {
    fn extension(self) -> &'static str {
        match self {
            Self::Webp => "webp",
            Self::Jpg => "jpg",
            Self::Png => "png",
        }
    }
}

fn replace_unicode_escapes(mut text: &str) -> String {
    let mut out = String::new();
    while let Some((lhs, rhs)) = text.split_once("\\u") {
        out.push_str(lhs);
        let Some((code, rhs)) = rhs.split_at_checked(4) else {
            text = rhs;
            out.push_str("\\u");
            continue;
        };

        let code = code.chars()
            .map(|ch| ch.to_digit(16))
            .try_fold(0, |acc, digit| Some(acc * 16 + digit?));

        match code.and_then(|c| char::from_u32(c)) {
            Some(ch) => { out.push(ch); }
            None => { out.push_str("\\u"); }
        }

        text = rhs;
    }
    out.push_str(text);
    out
}

impl Gallery {
    pub async fn load(client: &Client, id: u32) -> Result<Self> {
        let url = format!("https://nhentai.net/g/{id}");
        log::trace!("Connecting to gallery: {url}");

        let text = client.get(&url)
            .send().await
            .with_context(ctx!("Failed to retrive gallery at {url}"))?
            .error_for_status()
            .with_context(ctx!("Received error from nhentai at {url}"))?
            .text().await
            .with_context(ctx!("Failed to read text at {url}"))?;

        let document = Html::parse_document(&text);

        let selector = Selector::parse("body > script").unwrap();
        let json = document.select(&selector)
            .find_map(|s| {
                let inner = s.inner_html();
                let json = inner.trim().strip_prefix("window._gallery = JSON.parse(\"")?;
                let (json, _) = json.split_once('"')?;
                Some(replace_unicode_escapes(&json))
            })
            .with_context(ctx!("Failed to find gallery json info"))?;

        serde_json::from_str(&json)
            .with_context(ctx!("Failed to parse gallery json info"))
    }

    async fn serialize_self(&self, out_path: &Path) {
        let Ok(json) = serde_json::to_vec_pretty(self) else {
            log::warn!("Failed to serialize gallery info");
            return;
        };
        let Ok(mut file) = fs::File::create(out_path).await else {
            log::warn!("Failed to create gallery info file at {out_path:?}");
            return;
        };
        let Ok(()) = file.write_all(&json).await else {
            log::warn!("Failed to write gallery info file at {out_path:?}");
            let _ = fs::remove_file(out_path).await;
            return;
        };
    }

    async fn download_page(
        &self,
        extension: &str,
        index: usize,
        out_path: &Path,
        client: &Client,
        overwrite: bool,
        gallery_exists: bool,
    ) -> Result<()> {
        let server_no = rand::rng().random_range(1..=4);
        let filename = format!("{index}.{extension}");
        let url = format!("https://i{server_no}.nhentai.net/galleries/{}/{filename}", self.media_id);
        let path = out_path.join(filename);

        log::trace!("Downloading page #{index} from gallery: {} url: {url} path: {path:?}", self.id);

        if !overwrite && gallery_exists {
            if let Ok(true) = fs::try_exists(&path).await {
                log::trace!("Not downloading page #{index} from gallery: {} because it exists", self.id);
                return Ok(())
            } else {
                log::info!("Downloading missing page #{index} for gallery: {}", self.id);
            }
        }

        let bytes = client.get(&url)
            .send().await
            .with_context(ctx!("Failed to download page #{index} from gallery: {}", self.id))?
            .error_for_status()
            .with_context(ctx!("page #{index} from gallery: {} returned an error", self.id))?
            .bytes().await
            .with_context(ctx!("Failed to read page #{index} from gallery: {}", self.id))?;

        let mut file = fs::File::create(&path).await
            .with_context(ctx!("Failed to create file: {path:?}"))?;

        file.write_all(&bytes).await
            .with_context(ctx!("Failed to write to file: {path:?}"))
            .inspect_err(|_| _ = fs::remove_file(&path))
    }

    pub async fn download(&self,
        client: &Client,
        out_path: &Path,
        overwrite: bool,
        check_missing: bool
    ) -> Result<()> {
        let out_path = out_path.join(self.id.to_string());

        let exists = match fs::metadata(&out_path).await {
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Ok(m) if m.file_type().is_dir() => true,
            e @ Err(_) => { return e.with_context(ctx!("Cannot read gallery directory {out_path:?}")).map(|_| ()); }
            Ok(_) => anyhow::bail!("Cannot create gallery directory {out_path:?} a file is already present")
        };

        if !exists {
            fs::create_dir_all(&out_path).await
                .with_context(ctx!("Failed to create gallery directory {out_path:?}"))?;
        } else if !check_missing {
            log::debug!("Gallery folder {out_path:?} is already present do not attempt to download missing pages (check_missing == false)");
            return Ok(());
        }

        let gallery_info_path = out_path.join("gallery.json");
        let gallery_info_exists = fs::try_exists(&gallery_info_path).await.unwrap_or(false);
        self.serialize_self(&gallery_info_path).await;

        stream::iter(self.images.pages.iter().enumerate())
            .for_each_concurrent(5, async |(i, ext)| {
                let i = i + 1;
                let res = self.download_page(ext.extension(), i, &out_path, client, overwrite, gallery_info_exists).await;
                if let Err(e) = res {
                    log::warn!("Couldn't download page #{i} from gallery {}", self.id);
                    log::warn!("Error: {e}");
                }
            })
            .await;

        Ok(())
    }
}
