//! Lazy, bounded cover cache for home cards, with native Kitty images and
//! actual image pixels via halfblocks on other terminals.

use super::cover::HalfblockCover;
use crate::db::cache::{self, Anime};
use image::DynamicImage;
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
    Resize, StatefulImage,
};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinHandle,
};

const CAPACITY: usize = 128;
const MAX_BYTES: usize = 4 * 1024 * 1024;

struct Entry {
    image: Option<DynamicImage>,
    covers: HashMap<(u16, u16), Box<dyn StatefulProtocol>>,
    task: Option<JoinHandle<()>>,
    retry_at: Option<Instant>,
    touched: u64,
}

pub struct PosterCache {
    entries: HashMap<i64, Entry>,
    tx: mpsc::Sender<(i64, Option<DynamicImage>)>,
    rx: mpsc::Receiver<(i64, Option<DynamicImage>)>,
    pool: SqlitePool,
    client: reqwest::Client,
    permits: Arc<Semaphore>,
    clock: u64,
    picker: Picker,
    overlay: bool,
}

impl PosterCache {
    pub fn new(pool: SqlitePool) -> Result<Self, reqwest::Error> {
        let (tx, rx) = mpsc::channel(CAPACITY);
        Ok(Self {
            entries: HashMap::new(),
            tx,
            rx,
            pool,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()?,
            permits: Arc::new(Semaphore::new(4)),
            clock: 0,
            // Two square image pixels per terminal cell, using halfblocks only.
            picker: Picker::new((1, 2)),
            overlay: false,
        })
    }

    pub fn configure_terminal(&mut self, terminal: &Picker) {
        self.picker = Picker::new(terminal.font_size);
        self.picker.is_tmux = terminal.is_tmux;
        if terminal.protocol_type == ProtocolType::Kitty {
            self.picker.protocol_type = ProtocolType::Kitty;
        }
    }

    pub fn set_overlay(&mut self, overlay: bool) {
        self.overlay = overlay;
    }

    /// Two title lines and a one-cell border surround a 2:3 portrait poster.
    pub fn card_height(&self, width: u16) -> u16 {
        let (font_width, font_height) = self.picker.font_size;
        let pixels = u32::from(width.saturating_sub(2)) * u32::from(font_width) * 3;
        pixels
            .div_ceil(u32::from(font_height).max(1) * 2)
            .clamp(1, 60) as u16
            + 4
    }

    pub fn poll(&mut self) {
        while let Ok((id, image)) = self.rx.try_recv() {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.task = None;
                entry.image = image;
                entry.retry_at = entry
                    .image
                    .is_none()
                    .then(|| Instant::now() + Duration::from_secs(60));
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, anime: &Anime) {
        if area.is_empty() {
            return;
        }
        self.clock += 1;
        self.request(anime);
        if let Some(entry) = self.entries.get_mut(&anime.id) {
            entry.touched = self.clock;
            if let Some(image) = &entry.image {
                // Do not consume a Kitty transmission that an overlay Clear
                // might discard before the terminal receives this frame.
                if self.overlay && self.picker.protocol_type == ProtocolType::Kitty {
                    frame.render_widget(
                        HalfblockCover {
                            anime_id: anime.id,
                            title: anime.display_title(),
                        },
                        area,
                    );
                    return;
                }
                let size = (area.width, area.height);
                if !entry.covers.contains_key(&size) && entry.covers.len() >= 2 {
                    entry.covers.clear();
                }
                let cover = entry.covers.entry(size).or_insert_with(|| {
                    let (fw, fh) = self.picker.font_size;
                    let fitted = image.resize_to_fill(
                        u32::from(area.width) * u32::from(fw).max(1),
                        u32::from(area.height) * u32::from(fh).max(1),
                        image::imageops::FilterType::Triangle,
                    );
                    self.picker.new_resize_protocol(fitted)
                });
                frame.render_stateful_widget(
                    StatefulImage::new(None).resize(Resize::Fit(None)),
                    area,
                    cover,
                );
                return;
            }
        }
        frame.render_widget(
            HalfblockCover {
                anime_id: anime.id,
                title: anime.display_title(),
            },
            area,
        );
    }

    fn request(&mut self, anime: &Anime) {
        if let Some(entry) = self.entries.get(&anime.id) {
            if entry.retry_at.is_none_or(|retry| Instant::now() < retry) {
                return;
            }
            self.entries.remove(&anime.id);
        }
        if self.entries.len() >= CAPACITY {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| entry.task.is_none())
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(&id, _)| id);
            let Some(id) = oldest else {
                return;
            };
            self.entries.remove(&id);
        }
        let anime = anime.clone();
        let id = anime.id;
        let pool = self.pool.clone();
        let client = self.client.clone();
        let permits = self.permits.clone();
        let tx = self.tx.clone();
        let task = tokio::spawn(async move {
            let Ok(_permit) = permits.acquire_owned().await else {
                return;
            };
            let image = load_cover(&anime, &pool, &client).await;
            let _ = tx.send((id, image)).await;
        });
        self.entries.insert(
            id,
            Entry {
                image: None,
                covers: HashMap::new(),
                task: Some(task),
                retry_at: None,
                touched: self.clock,
            },
        );
    }
}

impl Drop for PosterCache {
    fn drop(&mut self) {
        for entry in self.entries.values() {
            if let Some(task) = &entry.task {
                task.abort();
            }
        }
    }
}

async fn decode(bytes: Vec<u8>) -> Option<DynamicImage> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        Some(reader.decode().ok()?.thumbnail(256, 384))
    })
    .await
    .ok()
    .flatten()
}

async fn load_cover(
    anime: &Anime,
    pool: &SqlitePool,
    client: &reqwest::Client,
) -> Option<DynamicImage> {
    // Re-read the blob so revisiting an evicted card never needs a network fetch.
    let blob = if anime.cover_blob.is_some() {
        anime.cover_blob.clone()
    } else {
        sqlx::query_scalar::<_, Option<Vec<u8>>>("SELECT cover_blob FROM anime WHERE id = ?")
            .bind(anime.id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten()
    };
    if let Some(bytes) = blob {
        if let Some(image) = decode(bytes).await {
            return Some(image);
        }
    }
    let mut response = client
        .get(anime.cover_url.as_ref()?)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    let image = decode(bytes.clone()).await?;
    let _ = cache::store_cover_blob(pool, anime.id, &bytes).await;
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, style::Color, Terminal};

    fn anime() -> Anime {
        serde_json::from_value(serde_json::json!({
            "id": 1, "title_romaji": "Test", "genres": "[]", "has_dub": 0, "updated_at": 0
        }))
        .unwrap()
    }

    fn png() -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            12,
            18,
            image::Rgb([240, 30, 10]),
        ));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[tokio::test]
    async fn cached_blob_renders_pixels_at_multiple_sizes_without_network() {
        let pool = crate::db::init(":memory:").await.unwrap();
        let anime = anime();
        cache::upsert_anime(&pool, &anime).await.unwrap();
        cache::store_cover_blob(&pool, anime.id, &png())
            .await
            .unwrap();
        let mut posters = PosterCache::new(pool).unwrap();
        posters.request(&anime);
        posters.request(&anime);
        assert_eq!(posters.entries.len(), 1);
        let (id, image) = tokio::time::timeout(Duration::from_secs(5), posters.rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(image.is_some());
        posters.tx.send((id, image)).await.unwrap();
        posters.poll();
        let mut terminal = Terminal::new(TestBackend::new(30, 14)).unwrap();
        terminal
            .draw(|frame| {
                posters.render(frame, Rect::new(0, 0, 8, 8), &anime);
                posters.render(frame, Rect::new(12, 0, 12, 12), &anime);
            })
            .unwrap();
        for x in [0, 12] {
            assert!(
                (x..x + 8).any(|x| {
                    (0..8)
                        .any(|y| terminal.backend().buffer()[(x, y)].fg == Color::Rgb(240, 30, 10))
                }),
                "cached image pixels missing at x={x}"
            );
        }
    }

    #[tokio::test]
    async fn kitty_transmission_survives_initial_loading_under_an_overlay() {
        let pool = crate::db::init(":memory:").await.unwrap();
        let mut posters = PosterCache::new(pool).unwrap();
        let mut picker = Picker::new((8, 16));
        picker.protocol_type = ProtocolType::Kitty;
        posters.configure_terminal(&picker);
        assert_eq!(posters.card_height(22), 19);
        posters.entries.insert(
            1,
            Entry {
                image: decode(png()).await,
                covers: HashMap::new(),
                task: None,
                retry_at: None,
                touched: 0,
            },
        );
        let anime = anime();
        let mut terminal = Terminal::new(TestBackend::new(8, 8)).unwrap();
        posters.set_overlay(true);
        terminal
            .draw(|frame| {
                let area = frame.area();
                posters.render(frame, area, &anime);
                frame.render_widget(ratatui::widgets::Clear, area);
            })
            .unwrap();
        assert!(posters.entries[&1].covers.is_empty());
        posters.set_overlay(false);
        terminal
            .draw(|frame| posters.render(frame, frame.area(), &anime))
            .unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.symbol().contains("\x1b_G")));
    }

    #[tokio::test]
    async fn invalid_images_fall_back_and_wait_before_retrying() {
        assert!(decode(vec![0, 1, 2]).await.is_none());
        assert!(decode(vec![0; MAX_BYTES + 1]).await.is_none());
        let pool = crate::db::init(":memory:").await.unwrap();
        let mut posters = PosterCache::new(pool).unwrap();
        let anime = anime();
        posters.request(&anime);
        let result = tokio::time::timeout(Duration::from_secs(5), posters.rx.recv())
            .await
            .unwrap()
            .unwrap();
        posters.tx.send(result).await.unwrap();
        posters.poll();
        posters.request(&anime);
        let entry = &posters.entries[&anime.id];
        assert!(entry.task.is_none());
        assert!(entry.retry_at.unwrap() > Instant::now());
        let mut terminal = Terminal::new(TestBackend::new(10, 8)).unwrap();
        terminal
            .draw(|frame| posters.render(frame, frame.area(), &anime))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "▓");
    }

    #[tokio::test]
    async fn cache_evicts_oldest_completed_entry_and_stays_bounded() {
        let pool = crate::db::init(":memory:").await.unwrap();
        let mut posters = PosterCache::new(pool).unwrap();
        for id in 0..CAPACITY as i64 {
            posters.entries.insert(
                id,
                Entry {
                    image: None,
                    covers: HashMap::new(),
                    task: None,
                    retry_at: Some(Instant::now() + Duration::from_secs(60)),
                    touched: id as u64,
                },
            );
        }
        let mut anime = anime();
        anime.id = CAPACITY as i64;
        posters.request(&anime);
        assert_eq!(posters.entries.len(), CAPACITY);
        assert!(!posters.entries.contains_key(&0));
        assert!(posters.entries.contains_key(&anime.id));
    }
}
