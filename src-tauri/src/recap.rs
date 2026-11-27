use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenEvent {
    pub timestamp: u64,
    pub track: Track,
    pub listened_sec: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecapPeriod {
    Day,
    Week,
    Month,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapArtistStat {
    pub name: String,
    pub plays: u32,
    pub total_sec: u64,
    pub artwork: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapTrackStat {
    pub track: Track,
    pub plays: u32,
    pub total_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapGenreStat {
    pub genre: String,
    pub plays: u32,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapSummary {
    pub badge: String,
    pub headline: String,
    pub description: String,
    pub accent_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapDiscovery {
    pub artist: String,
    pub track: Track,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningRecap {
    pub period: RecapPeriod,
    pub total_plays: u32,
    pub unique_tracks: u32,
    pub unique_artists: u32,
    pub total_sec: u64,
    pub top_genre: Option<String>,
    pub genres: Vec<RecapGenreStat>,
    pub top_artists: Vec<RecapArtistStat>,
    pub top_tracks: Vec<RecapTrackStat>,
    pub summary: RecapSummary,
    #[serde(default)]
    pub hourly_plays: Vec<u32>,
    #[serde(default)]
    pub peak_hour: Option<u8>,
    #[serde(default)]
    pub streak_days: u32,
    #[serde(default)]
    pub discovery: Option<RecapDiscovery>,
}

pub struct RecapStore {
    path: PathBuf,
    events: Vec<ListenEvent>,
}

fn now_unix_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl RecapStore {
    pub fn load(app_data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&app_data_dir);
        let path = app_data_dir.join("listening_history.json");
        let events: Vec<ListenEvent> = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, events }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string(&self.events).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    pub fn record(&mut self, track: Track, listened_sec: u32) {
        let now = now_unix_sec();
        // Защита от дубля, если тот же трек записался повторно менее чем за 15 сек
        if let Some(last) = self.events.last() {
            if last.track.id == track.id && now.saturating_sub(last.timestamp) < 15 {
                return;
            }
        }
        self.events.push(ListenEvent {
            timestamp: now,
            track,
            listened_sec,
        });
        if self.events.len() > 10_000 {
            let excess = self.events.len() - 10_000;
            self.events.drain(0..excess);
        }
        let _ = self.save();
    }

    pub fn build_recap(&self, period: RecapPeriod, tz_offset_min: i32) -> ListeningRecap {
        let now = now_unix_sec();
        // tz_offset_min от JS getTimezoneOffset(): UTC - local (например -240 для UTC+4)
        let local_shift_sec = -(tz_offset_min as i64) * 60;
        let local_now = (now as i64 + local_shift_sec).max(0) as u64;

        let cutoff = match period {
            RecapPeriod::Day => {
                // Начало текущих локальных суток
                let start_of_local_day = local_now - (local_now % 86_400);
                (start_of_local_day as i64 - local_shift_sec).max(0) as u64
            }
            RecapPeriod::Week => now.saturating_sub(7 * 86_400),
            RecapPeriod::Month => now.saturating_sub(30 * 86_400),
            RecapPeriod::All => 0,
        };

        let filtered: Vec<&ListenEvent> = self
            .events
            .iter()
            .filter(|e| e.timestamp >= cutoff)
            .collect();

        if filtered.is_empty() {
            return ListeningRecap {
                period,
                total_plays: 0,
                unique_tracks: 0,
                unique_artists: 0,
                total_sec: 0,
                top_genre: None,
                genres: Vec::new(),
                top_artists: Vec::new(),
                top_tracks: Vec::new(),
                summary: RecapSummary {
                    badge: "Пока тихо".to_string(),
                    headline: "Здесь появятся твои итоги".to_string(),
                    description: "Послушай несколько треков (от 25 секунд), и Flowly соберёт твой персональный разбор вкуса, топ артистов и главных треков.".to_string(),
                    accent_color: "#6366f1".to_string(),
                },
                hourly_plays: vec![0; 24],
                peak_hour: None,
                streak_days: 0,
                discovery: None,
            };
        }

        let mut total_sec: u64 = 0;
        let mut night_plays: u32 = 0;
        let mut hourly_plays = vec![0u32; 24];
        let mut artist_map: HashMap<String, RecapArtistStat> = HashMap::new();
        let mut track_map: HashMap<String, RecapTrackStat> = HashMap::new();
        let mut genre_map: HashMap<String, u32> = HashMap::new();
        let mut genre_total: u32 = 0;

        for ev in &filtered {
            let sec = ev.listened_sec.max(ev.track.duration) as u64;
            total_sec += sec;

            let local_ts = (ev.timestamp as i64 + local_shift_sec).max(0) as u64;
            let hour = ((local_ts % 86_400) / 3600) as u8;
            if (hour as usize) < 24 {
                hourly_plays[hour as usize] += 1;
            }
            if hour >= 23 || hour < 5 {
                night_plays += 1;
            }

            // Главный артист (разбиваем коллабы для честного топа)
            let primary_artist = ev
                .track
                .artist
                .split(&[',', '&'][..])
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&ev.track.artist)
                .to_string();

            artist_map
                .entry(primary_artist.clone())
                .and_modify(|a| {
                    a.plays += 1;
                    a.total_sec += sec;
                })
                .or_insert_with(|| RecapArtistStat {
                    name: primary_artist,
                    plays: 1,
                    total_sec: sec,
                    artwork: ev.track.artwork.clone(),
                });

            track_map
                .entry(ev.track.id.clone())
                .and_modify(|t| {
                    t.plays += 1;
                    t.total_sec += sec;
                })
                .or_insert_with(|| RecapTrackStat {
                    track: ev.track.clone(),
                    plays: 1,
                    total_sec: sec,
                });

            if let Some(ref g) = ev.track.genre {
                let clean = g.trim().to_lowercase();
                if !clean.is_empty() {
                    *genre_map.entry(clean).or_insert(0) += 1;
                    genre_total += 1;
                }
            }
        }

        let total_plays = filtered.len() as u32;
        let unique_tracks = track_map.len() as u32;
        let unique_artists = artist_map.len() as u32;

        let mut top_artists: Vec<RecapArtistStat> = artist_map.into_values().collect();
        top_artists.sort_by(|a, b| b.plays.cmp(&a.plays).then_with(|| b.total_sec.cmp(&a.total_sec)));
        top_artists.truncate(8);

        let mut top_tracks: Vec<RecapTrackStat> = track_map.into_values().collect();
        top_tracks.sort_by(|a, b| b.plays.cmp(&a.plays).then_with(|| b.total_sec.cmp(&a.total_sec)));
        top_tracks.truncate(15);

        let mut genre_entries: Vec<(String, u32)> = genre_map.into_iter().collect();
        genre_entries.sort_by(|a, b| b.1.cmp(&a.1));
        let genres: Vec<RecapGenreStat> = genre_entries
            .into_iter()
            .take(6)
            .map(|(genre, plays)| {
                let percent = if genre_total > 0 {
                    ((plays as f32 / genre_total as f32) * 100.0).round() as u8
                } else {
                    0
                };
                RecapGenreStat {
                    genre,
                    plays,
                    percent,
                }
            })
            .collect();

        let top_genre = genres.first().map(|g| g.genre.clone());
        let accent_color = top_tracks
            .first()
            .map(|t| t.track.accent_color.clone())
            .unwrap_or_else(|| "#6366f1".to_string());

        let summary = build_vibe_summary(
            period,
            total_plays,
            night_plays,
            &genres,
            &top_artists,
            &top_tracks,
            accent_color,
        );

        let peak_hour = hourly_plays
            .iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .max_by_key(|(_, &c)| c)
            .map(|(h, _)| h as u8);

        // Серия дней (Streak) по всей истории в локальном времени
        let mut active_days = std::collections::HashSet::<u64>::new();
        for ev in &self.events {
            let lts = (ev.timestamp as i64 + local_shift_sec).max(0) as u64;
            active_days.insert(lts / 86_400);
        }
        let today_day = local_now / 86_400;
        let mut streak_days: u32 = 0;
        let mut cursor = if active_days.contains(&today_day) {
            today_day
        } else if today_day > 0 && active_days.contains(&(today_day - 1)) {
            today_day - 1
        } else {
            u64::MAX
        };
        while cursor != u64::MAX && active_days.contains(&cursor) {
            streak_days += 1;
            if cursor == 0 {
                break;
            }
            cursor -= 1;
        }

        // Открытие периода: первый раз появился артист, которого не было до cutoff
        let discovery = if cutoff > 0 {
            let mut prior_artists = std::collections::HashSet::<String>::new();
            for ev in self.events.iter().filter(|e| e.timestamp < cutoff) {
                let pa = ev
                    .track
                    .artist
                    .split(&[',', '&'][..])
                    .next()
                    .map(str::trim)
                    .unwrap_or(&ev.track.artist)
                    .to_lowercase();
                if !pa.is_empty() {
                    prior_artists.insert(pa);
                }
            }
            top_tracks.iter().find_map(|ts| {
                let pa = ts
                    .track
                    .artist
                    .split(&[',', '&'][..])
                    .next()
                    .map(str::trim)
                    .unwrap_or(&ts.track.artist)
                    .to_string();
                if !pa.is_empty() && !prior_artists.contains(&pa.to_lowercase()) {
                    Some(RecapDiscovery {
                        artist: pa,
                        track: ts.track.clone(),
                    })
                } else {
                    None
                }
            })
        } else {
            None
        };

        ListeningRecap {
            period,
            total_plays,
            unique_tracks,
            unique_artists,
            total_sec,
            top_genre,
            genres,
            top_artists,
            top_tracks,
            summary,
            hourly_plays,
            peak_hour,
            streak_days,
            discovery,
        }
    }
}

fn build_vibe_summary(
    period: RecapPeriod,
    total_plays: u32,
    night_plays: u32,
    genres: &[RecapGenreStat],
    top_artists: &[RecapArtistStat],
    top_tracks: &[RecapTrackStat],
    accent_color: String,
) -> RecapSummary {
    let period_word = match period {
        RecapPeriod::Day => "Сегодня",
        RecapPeriod::Week => "На этой неделе",
        RecapPeriod::Month => "В этом месяце",
        RecapPeriod::All => "За всё время",
    };

    let a1 = top_artists.first().map(|a| a.name.as_str()).unwrap_or("любимых артистов");
    let a2 = top_artists.get(1).map(|a| a.name.as_str());
    let t1 = top_tracks.first().map(|t| t.track.title.as_str()).unwrap_or("главный трек");
    let t1_plays = top_tracks.first().map(|t| t.plays).unwrap_or(1);

    let artists_phrase = match a2 {
        Some(second) => format!("{} и {}", a1, second),
        None => a1.to_string(),
    };

    let top_g = genres.first().map(|g| g.genre.as_str()).unwrap_or("");
    let top_pct = genres.first().map(|g| g.percent).unwrap_or(0);

    let is_alt_nefor = matches!(
        top_g,
        "alternative" | "rock" | "punk" | "post-punk" | "metal" | "emo" | "grunge" | "indie" | "shoegaze" | "rusrock" | "hardrock"
    ) || top_g.contains("rock") || top_g.contains("punk") || top_g.contains("metal") || top_g.contains("alt");

    let is_rap = matches!(
        top_g,
        "rap" | "hip-hop" | "hiphop" | "trap" | "phonk" | "drill" | "rusrap" | "cloud"
    ) || top_g.contains("rap") || top_g.contains("hip");

    let is_electronic = matches!(
        top_g,
        "electronic" | "dance" | "house" | "techno" | "edm" | "dnb" | "synthwave" | "trance" | "electro"
    ) || top_g.contains("electr") || top_g.contains("house") || top_g.contains("techno");

    let is_chill = matches!(
        top_g,
        "lofi" | "lo-fi" | "ambient" | "chill" | "jazz" | "classical" | "acoustic" | "relax"
    );

    let is_pop = matches!(top_g, "pop" | "ruspop" | "rnb" | "soul" | "disco" | "kpop");

    let pick = |n: usize| -> usize {
        if n == 0 {
            0
        } else {
            rand::random::<usize>() % n
        }
    };

    let (badge, headline, mut desc) = if genres.len() >= 5 && top_pct < 35 {
        let variants = [
            (
                "Меломан 🎭",
                format!("{} у тебя полный жанровый микс", period_word),
                format!(
                    "В ротации сразу {} разных жанров — и при этом чаще всего в наушниках играли {}, а трек «{}» выделялся среди остальных.",
                    genres.len(), artists_phrase, t1
                ),
            ),
            (
                "Без границ 🌐",
                format!("{} ты успел послушать вообще всё", period_word),
                format!(
                    "{} жанров в одной ленте: от {} до неожиданных находок. Главным моментом стал трек «{}».",
                    genres.len(), artists_phrase, t1
                ),
            ),
            (
                "Музыкальный хамелеон 🎨",
                format!("{} настроение менялось с каждым треком", period_word),
                format!(
                    "Никаких рамок ({} направлений), в фаворитах — {}, а «{}» звучал ярче всего.",
                    genres.len(), artists_phrase, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else if is_alt_nefor {
        let variants = [
            (
                "Нефорский вайб 🖤",
                format!("{} ты стопроцентный нефор", period_word),
                format!(
                    "В плейлисте рубили {} (жанр {} занял {}%), а «{}» стал главным гимном.",
                    artists_phrase, top_g, top_pct, t1
                ),
            ),
            (
                "Гитары и перегруз 🎸",
                format!("{} эфир держали плотные риффы", period_word),
                format!(
                    "{} сделали всю атмосферу — {}% времени играл {}, особенно запомнился «{}».",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
            (
                "Андерграунд режим 🕸️",
                format!("{} всё звучало максимально трушно", period_word),
                format!(
                    "Тяжёлый/альтернативный настрой от {} ({}% в {}), а «{}» крутился на главном месте.",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
            (
                "Бунт в наушниках ⚡",
                format!("{} день под знаком альтернативы", period_word),
                format!(
                    "Главный саундтрек — {}, жанр {} забрал {}%, и «{}» попал прямо в нерв.",
                    artists_phrase, top_g, top_pct, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else if is_rap {
        let variants = [
            (
                "Уличный бит 🔥",
                format!("{} день прошёл на плотном бите", period_word),
                format!(
                    "Главные на фите — {}, жанр {} забрал {}% эфира, а трек «{}» задавал темп.",
                    artists_phrase, top_g, top_pct, t1
                ),
            ),
            (
                "Кач и 808-е 🔊",
                format!("{} басы не замолкали ни на минуту", period_word),
                format!(
                    "В топе качали {} ({}% — {}), а «{}» стал главным бэнгером.",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
            (
                "Флоу на максимуме 🎤",
                format!("{} всё звучало на уверенном флоу", period_word),
                format!(
                    "Больше всего в очереди держались {}, и трек «{}» идеально попал в настроение.",
                    artists_phrase, t1
                ),
            ),
            (
                "Плотнейший вайб 💯",
                format!("{} в колонках был чистый хип-хоп", period_word),
                format!(
                    "{}% эфира под {}, лидируют {}, а «{}» звучал громче остальных.",
                    top_pct, top_g, artists_phrase, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else if is_electronic {
        let variants = [
            (
                "Ночной разгон ⚡",
                format!("{} максимум электроники и пульса", period_word),
                format!(
                    "Ритм держали {} ({}% в стиле {}), а «{}» не отпускал из очереди.",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
            (
                "Неоновый рейв 🌆",
                format!("{} всё двигалось в ровном BPM", period_word),
                format!(
                    "За синтезаторы и драйв отвечали {}, а трек «{}» стал главным разгоном.",
                    artists_phrase, t1
                ),
            ),
            (
                "Пульс 128 BPM 🎛️",
                format!("{} чистая энергия синта и баса", period_word),
                format!(
                    "В приоритете {} ({}% — {}), и «{}» держал фокус до самого конца.",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else if is_chill {
        let variants = [
            (
                "Полный дзен 🌊",
                format!("{} на волне спокойного фокуса", period_word),
                format!(
                    "Атмосферу создавали {}, а трек «{}» идеально лёг на фон.",
                    artists_phrase, t1
                ),
            ),
            (
                "Мягкий поток ☁️",
                format!("{} всё звучало максимально спокойно", period_word),
                format!(
                    "Без спешки и суеты: в наушниках играли {}, а «{}» задавал уютный тон.",
                    artists_phrase, t1
                ),
            ),
            (
                "Режим тишины и фокуса 🍵",
                format!("{} музыка помогала держать баланс", period_word),
                format!(
                    "Тёплое звучание от {}, а трек «{}» оказался самым душевным.",
                    artists_phrase, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else if is_pop {
        let variants = [
            (
                "Чистая мелодика ✨",
                format!("{} под знаком лёгких хитов", period_word),
                format!(
                    "Больше всего звучали {}, а «{}» точно засел в голове.",
                    artists_phrase, t1
                ),
            ),
            (
                "Припев на повторе 🪩",
                format!("{} у тебя сплошные бэнгеры", period_word),
                format!(
                    "Вайб задавали {} ({}% в {}), а трек «{}» хотелось подпевать.",
                    artists_phrase, top_pct, top_g, t1
                ),
            ),
            (
                "Яркий саунд 💫",
                format!("{} настроение было максимально лёгким", period_word),
                format!(
                    "В топе держались {}, а «{}» стал главным треком этого отрезка.",
                    artists_phrase, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    } else {
        let variants = [
            (
                "Личный топ 🎧",
                format!("{} у тебя свой особенный саундтрек", period_word),
                format!(
                    "Чаще всего в наушниках звучали {}, а трек «{}» возглавил твой личный чарт.",
                    artists_phrase, t1
                ),
            ),
            (
                "Своя волна 📻",
                format!("{} собралась интересная подборка", period_word),
                format!(
                    "Тон задавали {}, а «{}» оказался в центре внимания.",
                    artists_phrase, t1
                ),
            ),
            (
                "Музыкальный дневник ✍️",
                format!("{} в плеере звучало только твоё", period_word),
                format!(
                    "Лидерами стали {}, а трек «{}» запомнился больше всех.",
                    artists_phrase, t1
                ),
            ),
        ];
        let v = &variants[pick(variants.len())];
        (v.0.to_string(), v.1.clone(), v.2.clone())
    };

    if t1_plays >= 4 {
        let repeat_notes = [
            format!(" Залип на «{}» — уже {} прослушиваний!", t1, t1_plays),
            format!(" Настоящая обсессия: «{}» проиграл {} раз(а).", t1, t1_plays),
            format!(" Кнопка повтора явно работала на «{}» ({} раза).", t1, t1_plays),
        ];
        desc.push_str(&repeat_notes[pick(repeat_notes.len())]);
    }
    if total_plays >= 3 && night_plays * 2 >= total_plays {
        let night_notes = [
            " И да, режим ночного дозора: большинство треков играли глубокой ночью.",
            " Судя по времени, твоё любимое время для музыки — глубокая ночь.",
            " Ночной плейлист в деле: основная часть прослушиваний пришлась на тёмное время суток.",
        ];
        desc.push_str(night_notes[pick(night_notes.len())]);
    }

    RecapSummary {
        badge,
        headline,
        description: desc,
        accent_color,
    }
}
