use kithara::assets::AssetStore;
use kithara::bufpool::{pool_schema, OverallBudget, PoolConfig, PoolRegion};
use kithara::events::TrackId;
use kithara::host::{Host, HostConfig, HostOwned};
use kithara::play::{
    CrossfadeCurve as KitharaCrossfadeCurve, CrossfadeSettings, PlayError, PlayWorker,
    PlayWorkerConfig, PlayerConfig, PlayerImpl, Resource, ResourceConfig, ResourceSrc,
    SelectTransition, SelectionPlayback, SessionDuckingMode,
};
use thiserror::Error;

pool_schema! {
    AppPools {
        bytes: u8,
        samples: f32,
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Ошибка пула памяти аудио-движка: {0}")]
    Pool(String),
    #[error("Ошибка инициализации хоста Kithara: {0}")]
    Host(#[from] PlayError),
    #[error("Некорректный URL аудиоисточника: {0}")]
    InvalidUrl(String),
    #[error("Следующий трек не подготовлен для кроссфейда")]
    NoPreparedSlot,
    #[error("Некорректный канал эквалайзера: {0}")]
    InvalidEqChannel(String),
    #[error("Внутренняя ошибка runtime: {0}")]
    Runtime(String),
}

/// Кривая DJ-кроссфейда между слотами.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrossfadeCurve {
    EqualPower,
    SCurve,
    Linear,
}

impl From<CrossfadeCurve> for KitharaCrossfadeCurve {
    fn from(value: CrossfadeCurve) -> Self {
        match value {
            CrossfadeCurve::Linear => KitharaCrossfadeCurve::Linear,
            CrossfadeCurve::EqualPower | CrossfadeCurve::SCurve => KitharaCrossfadeCurve::EqualPower,
        }
    }
}

/// Пресет перехода (Уровень 1 и Уровень 2 Automix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MixPreset {
    Auto,
    Fade,
    Rise,
}

/// Канал / полоса 10-полосного эквалайзера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EqChannel {
    Band(usize),
    SubBass,    // 32 Hz
    Bass,       // 64 Hz
    LowMid,     // 125 Hz
    Mid,        // 500 Hz
    Presence,   // 2 kHz
    Brilliance, // 8 kHz
    Air,        // 16 kHz
}

impl EqChannel {
    fn band_index(self) -> Result<usize, EngineError> {
        match self {
            EqChannel::Band(idx) if idx < 10 => Ok(idx),
            EqChannel::Band(idx) => Err(EngineError::InvalidEqChannel(format!("band {}", idx))),
            EqChannel::SubBass => Ok(0),
            EqChannel::Bass => Ok(1),
            EqChannel::LowMid => Ok(2),
            EqChannel::Mid => Ok(4),
            EqChannel::Presence => Ok(6),
            EqChannel::Brilliance => Ok(8),
            EqChannel::Air => Ok(9),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct AudioPlaybackStatus {
    pub position: f64,
    pub duration: f64,
    pub is_playing: bool,
}

/// Проверка гармонической совместимости по колесу Камелота (например `8A` и `9A` или `8B`).
pub fn camelot_compatible(a: Option<&str>, b: Option<&str>) -> bool {
    let (Some(ka), Some(kb)) = (a.and_then(parse_camelot), b.and_then(parse_camelot)) else {
        // Если метаданных тональности нет — не блокируем плавный микс по BPM
        return true;
    };
    if ka.0 == kb.0 {
        return true; // Тот же номер (вкл. A <-> B)
    }
    if ka.1 == kb.1 {
        let diff = (ka.0 as i32 - kb.0 as i32).abs();
        return diff == 1 || diff == 11; // Соседние сектора круга 1..12
    }
    false
}

fn parse_camelot(raw: &str) -> Option<(u8, char)> {
    let s = raw.trim().to_uppercase();
    if s.len() < 2 {
        return None;
    }
    let letter = s.chars().last()?;
    if letter != 'A' && letter != 'B' {
        return None;
    }
    let num: u8 = s[..s.len() - 1].parse().ok()?;
    if (1..=12).contains(&num) {
        Some((num, letter))
    } else {
        None
    }
}

/// Обёртка над Kithara (`Host` + `PlayerImpl`) с 2 слотами для бесшовного DJ-микса.
pub struct KitharaEngine {
    _host: Host<AppPools>,
    player: HostOwned<PlayerImpl<AppPools>>,
    pools: PoolRegion<AppPools>,
    worker: PlayWorker<AppPools>,
    rt: tokio::runtime::Runtime,
    active_slot: usize,
    prepared_slot: Option<usize>,
    next_track_seq: u64,
    user_eq_bands: [f32; 10],
}

impl KitharaEngine {
    pub fn new() -> Result<Self, EngineError> {
        let pool_cfg = || PoolConfig::builder().max_buffers(128).build();
        let pools = AppPools::builder(OverallBudget(64 * 1024 * 1024))
            .bytes(pool_cfg())
            .samples(pool_cfg())
            .build()
            .map_err(|e| EngineError::Pool(format!("{e:?}")))?;

        let mut host = Host::<AppPools>::new(HostConfig::builder().build())?;
        let worker = PlayWorker::new(PlayWorkerConfig::builder(pools.clone()).build());

        let player_cfg = PlayerConfig::builder()
            .sample_rate(host.requested_sample_rate())
            .max_slots(2)
            .worker(worker.clone())
            .build();
        let player_impl = PlayerImpl::new(player_cfg);
        let player = host.insert(player_impl)?;
        player.reserve_slots(2);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| EngineError::Runtime(e.to_string()))?;

        Ok(Self {
            _host: host,
            player,
            pools,
            worker,
            rt,
            active_slot: 0,
            prepared_slot: None,
            next_track_seq: 1,
            user_eq_bands: [0.0; 10],
        })
    }

    fn build_resource(&mut self, url: &str) -> Result<(Resource, TrackId), EngineError> {
        let src = ResourceSrc::parse(url).map_err(|e| EngineError::InvalidUrl(e.to_string()))?;
        let store = AssetStore::builder(self.pools.clone()).build();
        let config: ResourceConfig<AppPools> = ResourceConfig::for_src(src)
            .store(store)
            .worker(self.worker.clone())
            .build();

        let prepared = self.player.prepare_config(config)?;
        let resource = self
            .rt
            .block_on(Resource::new(prepared))
            .map_err(|e| EngineError::InvalidUrl(e.to_string()))?;

        let id = TrackId::from(self.next_track_seq);
        self.next_track_seq += 1;
        Ok((resource, id))
    }

    fn place_in_slot(&mut self, slot: usize, resource: Resource, id: TrackId) -> Result<(), EngineError> {
        if self.player.item_has_resource(slot) {
            self.player.replace_item(slot, resource, id)?;
        } else {
            self.player.insert(resource, id, Some(slot));
        }
        Ok(())
    }

    /// Загрузить трек в свободный слот и начать воспроизведение.
    pub fn load_and_play(&mut self, url: &str) -> Result<(), EngineError> {
        let (resource, id) = self.build_resource(url)?;
        let target_slot = self.active_slot;
        self.place_in_slot(target_slot, resource, id)?;
        self.prepared_slot = None;
        self.player.set_rate(1.0);
        self.player.select_item(target_slot, SelectionPlayback::Play)?;
        Ok(())
    }

    /// Подготовить следующий трек в другом слоте (для DJ-микса).
    pub fn prepare_next(&mut self, url: &str) -> Result<(), EngineError> {
        let (resource, id) = self.build_resource(url)?;
        let standby_slot = if self.active_slot == 0 { 1 } else { 0 };
        self.place_in_slot(standby_slot, resource, id)?;
        self.prepared_slot = Some(standby_slot);
        Ok(())
    }

    /// Запустить DJ-кроссфейд между текущим и подготовленным треком.
    pub fn crossfade(&mut self, duration_secs: f32, curve: CrossfadeCurve) -> Result<(), EngineError> {
        let next_slot = self.prepared_slot.ok_or(EngineError::NoPreparedSlot)?;
        let dur = duration_secs.clamp(0.5, 12.0);
        let position = if curve == CrossfadeCurve::SCurve { 0.52 } else { 0.5 };
        let settings = CrossfadeSettings::new(dur, curve.into(), 1.0, position)?;
        self.player.select_item_with_crossfade(
            next_slot,
            SelectTransition {
                crossfade: settings,
                playback: SelectionPlayback::Play,
            },
        )?;
        self.active_slot = next_slot;
        self.prepared_slot = None;
        Ok(())
    }

    /// Умный переход с учётом пресета (`Auto`, `Fade`, `Rise`), BPM и тональности.
    pub fn crossfade_with_preset(
        &mut self,
        duration_secs: f32,
        curve: CrossfadeCurve,
        preset: MixPreset,
        bpm_a: Option<f32>,
        bpm_b: Option<f32>,
        key_a: Option<&str>,
        key_b: Option<&str>,
    ) -> Result<(), EngineError> {
        match preset {
            MixPreset::Fade => self.crossfade(duration_secs, curve),
            MixPreset::Rise => self.trigger_rise(duration_secs),
            MixPreset::Auto => {
                let bpm_close = match (bpm_a, bpm_b) {
                    (Some(a), Some(b)) if a > 40.0 && b > 40.0 => (a - b).abs() < 8.0,
                    _ => false,
                };
                let key_ok = camelot_compatible(key_a, key_b);
                if bpm_close && key_ok {
                    if let (Some(a), Some(b)) = (bpm_a, bpm_b) {
                        let ratio = (a / b).clamp(0.94, 1.06);
                        self.player.set_rate(ratio);
                    }
                    let res = self.crossfade(duration_secs.max(5.0), CrossfadeCurve::EqualPower);
                    let player = self.player.clone();
                    let reset_ms = (duration_secs.max(5.0) * 1000.0) as u64;
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(reset_ms));
                        player.set_rate(1.0);
                    });
                    res
                } else {
                    self.trigger_rise(duration_secs.min(3.5))
                }
            }
        }
    }

    fn trigger_rise(&mut self, duration_secs: f32) -> Result<(), EngineError> {
        // Нарастающий подъём ВЧ и лёгкий срез низа на момент перехода, затем возврат EQ
        let _ = self.player.set_eq_gain(0, (self.user_eq_bands[0] - 4.0).clamp(-24.0, 12.0));
        let _ = self.player.set_eq_gain(8, (self.user_eq_bands[8] + 3.5).clamp(-24.0, 12.0));
        let _ = self.player.set_eq_gain(9, (self.user_eq_bands[9] + 4.0).clamp(-24.0, 12.0));

        let dur = duration_secs.clamp(1.0, 4.5);
        self.crossfade(dur, CrossfadeCurve::SCurve)?;

        let saved_bands = self.user_eq_bands;
        let player = self.player.clone();
        let delay_ms = (dur * 1000.0) as u64;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            for (idx, gain) in saved_bands.iter().enumerate() {
                let _ = player.set_eq_gain(idx, *gain);
            }
        });
        Ok(())
    }

    /// Управление EQ (например, для Gaming-пресета или 10-полосного эквалайзера).
    pub fn set_eq(&mut self, channel: EqChannel, gain_db: f32) -> Result<(), EngineError> {
        let band = channel.band_index()?;
        let clamped = gain_db.clamp(-24.0, 12.0);
        self.user_eq_bands[band] = clamped;
        self.player.set_eq_gain(band, clamped)?;
        Ok(())
    }

    /// Синхронизация всех 10 полос EQ.
    pub fn sync_eq_bands(&mut self, bands: &[f32]) -> Result<(), EngineError> {
        for (idx, gain) in bands.iter().take(10).enumerate() {
            let clamped = gain.clamp(-24.0, 12.0);
            self.user_eq_bands[idx] = clamped;
            self.player.set_eq_gain(idx, clamped)?;
        }
        Ok(())
    }

    /// Пауза.
    pub fn pause(&mut self) -> Result<(), EngineError> {
        self.player.pause();
        Ok(())
    }

    /// Возобновление.
    pub fn resume(&mut self) -> Result<(), EngineError> {
        self.player.play();
        Ok(())
    }

    /// Перемотка текущего трека.
    pub fn seek(&mut self, seconds: f64) -> Result<(), EngineError> {
        self.player.seek_seconds(seconds.max(0.0))?;
        Ok(())
    }

    /// Громкость (0.0..=1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.player.set_volume(volume.clamp(0.0, 1.0));
    }

    /// Приглушение звука (например, для диктора радио).
    pub fn set_ducking(&mut self, ducked: bool) -> Result<(), EngineError> {
        let mode = if ducked {
            SessionDuckingMode::Hard
        } else {
            SessionDuckingMode::Off
        };
        self.player.set_session_ducking(mode)?;
        Ok(())
    }

    pub fn status(&self) -> AudioPlaybackStatus {
        AudioPlaybackStatus {
            position: self.player.position_seconds().unwrap_or(0.0),
            duration: self.player.duration_seconds().unwrap_or(0.0),
            is_playing: self.player.is_playing(),
        }
    }
}
