//! Dalele OpenStreetMap: descărcare pe fire de lucru, cache pe disc și în memorie
//! (LRU, max. 400 de texturi), cu dala părinte afișată cât timp se încarcă cea curentă.
//!
//! Politica OSM: User-Agent propriu, cache local, fără descărcări în masă.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub type TileKey = (u8, u32, u32);

const MAX_TEXTURES: usize = 400;
const ERROR_RETRY: Duration = Duration::from_secs(15);
const WORKERS: usize = 4;
/// Peste această mărime, cache-ul de pe disc se curăță la pornire…
const DISK_MAX_BYTES: u64 = 500 * 1024 * 1024;
/// …ștergând dalele cele mai vechi până sub acest prag.
const DISK_KEEP_BYTES: u64 = 350 * 1024 * 1024;
pub const USER_AGENT: &str = concat!("HidroCalc/", env!("CARGO_PKG_VERSION"), " (desktop app)");

enum Entry {
    Ready(TextureHandle, u64),
    Error(Instant),
}

pub struct TileCache {
    entries: HashMap<TileKey, Entry>,
    inflight: HashSet<TileKey>,
    req_tx: Sender<TileKey>,
    /// `None` = cererea a fost abandonată (dala nu mai era pe ecran când i-a venit rândul).
    res_rx: Receiver<(TileKey, Option<Result<ColorImage, String>>)>,
    /// Dalele cerute în cadrul curent; firele de lucru sar peste cele care nu mai sunt aici,
    /// ca la deplasarea rapidă a hărții să nu se descarce dale deja ieșite din ecran.
    wanted: Arc<Mutex<HashSet<TileKey>>>,
    clock: u64,
}

fn disk_dir() -> Option<PathBuf> {
    crate::db::cache_dir("osm-tiles")
}

fn load(key: TileKey, agent: &ureq::Agent) -> Result<ColorImage, String> {
    let (z, x, y) = key;
    let path = disk_dir().map(|d| d.join(format!("{z}/{x}/{y}.png")));
    let bytes = match path.as_ref().and_then(|p| std::fs::read(p).ok()) {
        Some(b) => b,
        None => {
            let url = format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png");
            let resp = agent.get(&url).call().map_err(|e| e.to_string())?;
            let mut b = Vec::new();
            std::io::Read::read_to_end(&mut resp.into_reader(), &mut b).map_err(|e| e.to_string())?;
            if let Some(p) = &path {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                    let tmp = p.with_extension("tmp");
                    if std::fs::write(&tmp, &b).is_ok() {
                        let _ = std::fs::rename(&tmp, p);
                    }
                }
            }
            b
        }
    };
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()))
}

/// Fișierele din cache (recursiv), cu mărimea și data ultimei modificări.
fn disk_files(dir: &std::path::Path, out: &mut Vec<(PathBuf, u64, std::time::SystemTime)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            disk_files(&e.path(), out);
        } else {
            out.push((e.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
        }
    }
}

/// Ține cache-ul de dale de pe disc sub o limită: peste `max`, șterge dalele cele mai vechi
/// până sub `keep`. Întoarce numărul de fișiere șterse.
fn prune_disk_cache(dir: &std::path::Path, max: u64, keep: u64) -> usize {
    let mut files = Vec::new();
    disk_files(dir, &mut files);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= max {
        return 0;
    }
    files.sort_by_key(|f| f.2);
    let mut removed = 0;
    for (path, size, _) in files {
        if total <= keep {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
            removed += 1;
        }
    }
    removed
}

impl TileCache {
    pub fn new(ctx: &egui::Context) -> Self {
        // Curățarea cache-ului de pe disc, o dată la pornire, pe un fir separat.
        if let Some(dir) = disk_dir() {
            std::thread::spawn(move || {
                let n = prune_disk_cache(&dir, DISK_MAX_BYTES, DISK_KEEP_BYTES);
                if n > 0 {
                    log::info!("cache dale OSM: {n} fișiere vechi șterse");
                }
            });
        }
        let (req_tx, req_rx) = channel::<TileKey>();
        let (res_tx, res_rx) = channel();
        let req_rx = Arc::new(Mutex::new(req_rx));
        let wanted: Arc<Mutex<HashSet<TileKey>>> = Arc::default();
        for _ in 0..WORKERS {
            let wanted = Arc::clone(&wanted);
            let rx = Arc::clone(&req_rx);
            let tx = res_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).user_agent(USER_AGENT).build();
                loop {
                    let key = match rx.lock() {
                        Ok(r) => match r.recv() {
                            Ok(k) => k,
                            Err(_) => return,
                        },
                        Err(_) => return,
                    };
                    let still_wanted = wanted.lock().map(|w| w.contains(&key)).unwrap_or(true);
                    let res = still_wanted.then(|| load(key, &agent));
                    if tx.send((key, res)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self { entries: HashMap::new(), inflight: HashSet::new(), req_tx, res_rx, wanted, clock: 0 }
    }

    /// Preia dalele sosite de la firele de lucru.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Ok(mut w) = self.wanted.lock() {
            w.clear();
        }
        while let Ok((key, res)) = self.res_rx.try_recv() {
            self.inflight.remove(&key);
            let Some(res) = res else { continue };
            self.clock += 1;
            let entry = match res {
                Ok(img) => Entry::Ready(ctx.load_texture(format!("osm-{}-{}-{}", key.0, key.1, key.2), img, TextureOptions::LINEAR), self.clock),
                Err(e) => {
                    log::warn!("dală OSM {key:?}: {e}");
                    Entry::Error(Instant::now())
                }
            };
            self.entries.insert(key, entry);
        }
        if self.entries.len() > MAX_TEXTURES {
            let mut ready: Vec<(TileKey, u64)> = self.entries.iter().filter_map(|(k, e)| if let Entry::Ready(_, t) = e { Some((*k, *t)) } else { None }).collect();
            ready.sort_by_key(|(_, t)| *t);
            for (k, _) in ready.into_iter().take(self.entries.len() - MAX_TEXTURES) {
                self.entries.remove(&k);
            }
        }
    }

    /// Textura dalei, dacă e încărcată; altfel o cere (o singură dată, cu reîncercare după eroare).
    pub fn get(&mut self, key: TileKey) -> Option<egui::TextureId> {
        self.clock += 1;
        let now = self.clock;
        match self.entries.get_mut(&key) {
            Some(Entry::Ready(tex, used)) => {
                *used = now;
                return Some(tex.id());
            }
            Some(Entry::Error(at)) if at.elapsed() < ERROR_RETRY => return None,
            _ => {}
        }
        if let Ok(mut w) = self.wanted.lock() {
            w.insert(key);
        }
        if self.inflight.insert(key) {
            let _ = self.req_tx.send(key);
        }
        None
    }

    /// Doar din cache (fără cerere), pentru dala părinte.
    pub fn peek(&self, key: TileKey) -> Option<egui::TextureId> {
        match self.entries.get(&key) {
            Some(Entry::Ready(tex, _)) => Some(tex.id()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_cache_is_pruned_oldest_first_down_to_the_keep_size() {
        let dir = std::env::temp_dir().join(format!("hidrocalc-tiles-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("14/1")).unwrap();
        for i in 0..5u64 {
            let p = dir.join(format!("14/1/{i}.png"));
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let t = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000 + i * 60);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        assert_eq!(prune_disk_cache(&dir, 1000, 300), 0, "sub limită nu se șterge nimic");
        assert_eq!(prune_disk_cache(&dir, 400, 300), 2);
        assert!(!dir.join("14/1/0.png").exists() && !dir.join("14/1/1.png").exists(), "cele mai vechi dispar primele");
        assert!(dir.join("14/1/4.png").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
