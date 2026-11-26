<script lang="ts">
  import { onMount } from 'svelte';
  import { CheckCircle2, XCircle, RefreshCw, KeyRound, LogOut, ExternalLink, Shield, Globe, Sparkles, Mic, Download, Cpu } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import type {
    ActivityConfig,
    NetworkLyricsSettings,
    Playlist,
    ProxyConfig,
    ProxyMode,
    RadioHostConfig,
    SourceKind,
    SourceStatus,
  } from '../types';

  interface Props {
    sources: SourceStatus[];
    enabledSources?: SourceKind[];
    onEnabledSourcesChange?: (next: SourceKind[]) => void;
    onSourcesChanged: (next: SourceStatus[]) => void;
    onOpenGuide?: () => void;
    atmosphereSummary?: string;
    onDynamicThemeChange?: (enabled: boolean, city: string) => void;
    activityConfig?: ActivityConfig;
    onActivityConfigChange?: (next: ActivityConfig) => void;
    playlists?: Playlist[];
    currentActivityLabel?: string;
    activeCssThemeId?: string;
    customThemeCss?: string;
    onApplyCssTheme?: (id: string, css: string) => void;
    radio?: RadioHostConfig;
    radioStatus?: string;
    onRadioChange?: (next: RadioHostConfig) => void;
    onTestRadio?: () => void;
    ramSaverEnabled?: boolean;
    onRamSaverChange?: (next: boolean) => void;
  }

  let {
    sources,
    enabledSources = ['yandex', 'spotify', 'soundcloud'],
    onEnabledSourcesChange = () => {},
    onSourcesChanged,
    onOpenGuide = () => {},
    atmosphereSummary = '',
    onDynamicThemeChange = () => {},
    activityConfig = {
      enabled: false,
      codingPlaylistId: '',
      codingEqPreset: 'Lo-Fi',
      codingEco: true,
      gamingEqPreset: 'Gaming',
      gamingEco: true,
      videoAutoPause: true,
      videoAutoResume: true,
      customGames: '',
    },
    onActivityConfigChange = () => {},
    playlists = [],
    currentActivityLabel = '',
    activeCssThemeId = 'default',
    customThemeCss = '',
    onApplyCssTheme = () => {},
    radio = { enabled: false, engine: 'edge', voice: 'male', frequency: 3, city: '' },
    radioStatus = '',
    onRadioChange = () => {},
    onTestRadio = () => {},
    ramSaverEnabled = false,
    onRamSaverChange = () => {},
  }: Props = $props();

  let installingPiper = $state(false);
  let piperInstallMsg = $state('');

  let updateStatus = $state<'idle' | 'checking' | 'available' | 'latest' | 'installing' | 'error'>('idle');
  let updateVersion = $state('');
  let updateNotes = $state('');
  let updateError = $state('');

  async function checkForOtaUpdate() {
    if (updateStatus === 'checking' || updateStatus === 'installing') return;
    updateStatus = 'checking';
    updateError = '';
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const upd = await check();
      if (upd) {
        updateVersion = upd.version;
        updateNotes = upd.body ?? '';
        updateStatus = 'available';
      } else {
        updateStatus = 'latest';
      }
    } catch (e) {
      updateError = String(e);
      updateStatus = 'error';
    }
  }

  async function installOtaUpdate() {
    if (updateStatus === 'installing') return;
    updateStatus = 'installing';
    updateError = '';
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const { relaunch } = await import('@tauri-apps/plugin-process');
      const upd = await check();
      if (upd) {
        await upd.downloadAndInstall();
        await relaunch();
      } else {
        updateStatus = 'latest';
      }
    } catch (e) {
      updateError = String(e);
      updateStatus = 'error';
    }
  }

  async function handleInstallPiper() {
    if (installingPiper) return;
    installingPiper = true;
    piperInstallMsg = 'Скачивание Piper и русских моделей (~130 МБ)...';
    try {
      piperInstallMsg = await api.installPiperTts();
    } catch (e) {
      piperInstallMsg = String(e);
    } finally {
      installingPiper = false;
    }
  }

  let settingsTab = $state<'services' | 'network' | 'appearance' | 'themes' | 'integrations'>('services');

  const settingsTabs: { id: 'services' | 'network' | 'appearance' | 'themes' | 'integrations'; label: string }[] = [
    { id: 'services', label: 'Сервисы и аккаунты' },
    { id: 'network', label: 'Сеть и тексты' },
    { id: 'appearance', label: 'Система и тема' },
    { id: 'themes', label: 'Магазин тем (CSS)' },
    { id: 'integrations', label: 'Интеграции и режимы' },
  ];

  interface CssThemePreset {
    id: string;
    name: string;
    author: string;
    desc: string;
    swatches: [string, string, string];
    css: string;
  }

  const cssThemeStore: CssThemePreset[] = [
    {
      id: 'default',
      name: 'Default Flowly',
      author: 'Flowly',
      desc: 'Классическая тёмная графика с адаптивным акцентом обложки.',
      swatches: ['#0b0b0e', '#131318', '#ff3b5c'],
      css: '',
    },
    {
      id: 'cyberpunk',
      name: 'Cyberpunk Neon',
      author: 'Flowly Studio',
      desc: 'Неоновый маджента-циан, глубокий фиолетовый графит и кибер-свечение.',
      swatches: ['#090614', '#160d2e', '#ff2a85'],
      css: `:root {
  --bg-graphite: #090614;
  --bg-surface: #120b24;
  --bg-elevated: #1c1236;
  --bg-hover: #271a4a;
  --accent: #ff2a85;
  --border-subtle: rgba(0, 240, 255, 0.16);
  --border-strong: rgba(255, 42, 133, 0.42);
}
.shell {
  box-shadow: inset 0 0 90px rgba(0, 240, 255, 0.06);
}
.t-play, .wave-go, .wave-start-btn {
  box-shadow: 0 0 18px rgba(255, 42, 133, 0.45);
}`,
    },
    {
      id: 'nordic',
      name: 'Nordic Frost',
      author: 'Flowly Studio',
      desc: 'Скандинавский минимализм: холодный сланец, ледяной голубой и мягкий контраст.',
      swatches: ['#121820', '#1b2430', '#5cc8ff'],
      css: `:root {
  --bg-graphite: #121820;
  --bg-surface: #19222e;
  --bg-elevated: #222e3d;
  --bg-hover: #2b3a4d;
  --accent: #5cc8ff;
  --text-primary: #eef4fa;
  --text-secondary: #b7c8da;
  --border-subtle: rgba(168, 213, 255, 0.12);
}`,
    },
    {
      id: 'sakura',
      name: 'Sakura Velvet',
      author: 'Flowly Studio',
      desc: 'Тёплая ночная сакура: бархатный сливовый фон и нежно-розовый акцент.',
      swatches: ['#140d14', '#211521', '#ff7eb6'],
      css: `:root {
  --bg-graphite: #140d14;
  --bg-surface: #1f131f;
  --bg-elevated: #2b1b2b;
  --bg-hover: #382438;
  --accent: #ff7eb6;
  --radius-md: 16px;
  --radius-lg: 20px;
  --border-subtle: rgba(255, 166, 209, 0.14);
}`,
    },
    {
      id: 'matrix',
      name: 'Matrix Terminal',
      author: 'Flowly Studio',
      desc: 'Фосфорный зелёный, чёткие границы и атмосфера ночного терминала.',
      swatches: ['#050a06', '#0b140d', '#22ff66'],
      css: `:root {
  --bg-graphite: #050a06;
  --bg-surface: #0a140d;
  --bg-elevated: #102015;
  --bg-hover: #172e1e;
  --accent: #22ff66;
  --text-primary: #dcffe6;
  --text-secondary: #86c99b;
  --border-subtle: rgba(34, 255, 102, 0.18);
  --radius-sm: 4px;
  --radius-md: 6px;
}`,
    },
    {
      id: 'sunset',
      name: 'Sunset Vinyl',
      author: 'Flowly Studio',
      desc: 'Тёплое янтарное золото, глубокий виниловый уголь и мягкие блики.',
      swatches: ['#120e0b', '#1d1712', '#f59e0b'],
      css: `:root {
  --bg-graphite: #120e0b;
  --bg-surface: #1c1611;
  --bg-elevated: #271f18;
  --bg-hover: #332920;
  --accent: #f59e0b;
  --text-primary: #fbf4ea;
  --text-secondary: #d4c2ad;
  --border-subtle: rgba(245, 158, 11, 0.15);
}`,
    },
  ];

  let cssDraft = $state('');
  let cssCopied = $state(false);
  $effect(() => {
    cssDraft = customThemeCss;
  });

  function insertCssSnippet(snippet: string) {
    cssDraft = cssDraft.trim() ? `${cssDraft.trimEnd()}\n${snippet}` : `:root {\n  ${snippet}\n}`;
  }

  const sourceLabels: { id: SourceKind; label: string }[] = [
    { id: 'yandex', label: 'Яндекс Музыка' },
    { id: 'spotify', label: 'Spotify' },
    { id: 'soundcloud', label: 'SoundCloud' },
  ];

  function toggleEnabledSource(id: SourceKind) {
    const exists = enabledSources.includes(id);
    if (exists) {
      if (enabledSources.length <= 1) return; // хотя бы один должен остаться
      onEnabledSourcesChange(enabledSources.filter((s) => s !== id));
    } else {
      onEnabledSourcesChange([...enabledSources, id]);
    }
  }

  let yandexToken = $state('');
  let spotifyClientId = $state('');
  let scToken = $state('');
  let scClientId = $state('');

  let busy = $state<Record<string, boolean>>({});
  let feedback = $state<Record<string, string>>({});
  let proxyDrafts = $state<Record<string, ProxyConfig>>({});

  let netLyrics = $state<NetworkLyricsSettings>({
    doh_enabled: true,
    doh_provider: 'cloudflare',
    spicy_lyrics_key: '',
    discord_rpc_enabled: true,
    discord_client_id: '',
    close_to_tray: false,
    dynamic_theme_enabled: false,
    weather_city: '',
  });
  let netLyricsFeedback = $state('');
  let dohLatencies = $state<Record<string, { ms?: number; err?: boolean }>>({});
  let checkingAllDoh = $state(false);

  const dohPlainServers: { id: string; name: string; url: string }[] = [
    { id: 'google', name: 'Google', url: 'dns.google/dns-query' },
    { id: 'cloudflare', name: 'Cloudflare', url: 'cloudflare-dns.com/dns-query' },
    { id: 'quad9', name: 'Quad9', url: 'dns.quad9.net/dns-query' },
    { id: 'controld', name: 'Control D', url: 'freedns.controld.com/p0' },
    { id: 'dns4eu', name: 'DNS4EU', url: 'unfiltered.joindns4.eu/dns-query' },
    { id: 'yandex', name: 'Яндекс', url: 'common.dot.dns.yandex.net/dns-query' },
  ];

  const dohGeoServers: { id: string; name: string; url: string }[] = [
    { id: 'xbox', name: 'XBOX', url: 'xbox-dns.ru/dns-query' },
    { id: 'comss', name: 'Comss', url: 'dns.comss.one/dns-query' },
    { id: 'dns-ai', name: 'DNS-AI', url: 'dns.dns-ai.ru/dns-query' },
    { id: 'bezmezhau', name: 'Bezmezhau', url: 'sync.bezmezhau.com/dns-query' },
    { id: 'geohide', name: 'GeoHide', url: 'geohide.cc/dns-query' },
    { id: 'malw', name: 'MALW', url: 'dns.malw.link/dns-query' },
    { id: 'webshift', name: 'WebShift', url: 'd.webshiftvpn.ru/dns-query' },
    { id: 'dagami', name: 'Dagami', url: 'dns.dagami.ru/dns-query' },
    { id: 'magicdns', name: 'MagicDNS', url: 'dns.magicdns.ru/dns-query' },
    { id: 'tamaev', name: 'tamaev-gifts.xyz', url: 'tamaev-gifts.xyz/dns-query' },
    { id: 'zombiez', name: 'zombiez.ru', url: 'dns.zombiez.ru/dns-query' },
    { id: 'wlrus', name: 'wlrus.cfd', url: 'dns.wlrus.cfd/dns-query' },
    { id: 'magicinfo', name: 'magicinfo.art', url: 'api.magicinfo.art/dns-query' },
    { id: 'quicbest', name: 'quic.best', url: 'w.quic.best/dns-query' },
  ];

  async function checkAllDohServers() {
    checkingAllDoh = true;
    const all = [...dohPlainServers, ...dohGeoServers];
    await Promise.all(
      all.map(async (srv) => {
        try {
          const ms = await api.testDohServer(srv.id);
          dohLatencies = { ...dohLatencies, [srv.id]: { ms } };
        } catch {
          dohLatencies = { ...dohLatencies, [srv.id]: { err: true } };
        }
      })
    );
    checkingAllDoh = false;
  }

  onMount(async () => {
    try {
      netLyrics = await api.getNetworkLyricsSettings();
    } catch {
      // ignore
    }
  });

  async function saveNetLyrics(next?: Partial<NetworkLyricsSettings>) {
    if (next) {
      netLyrics = { ...netLyrics, ...next };
    }
    busy.netLyrics = true;
    netLyricsFeedback = '';
    try {
      netLyrics = await api.updateNetworkLyricsSettings(netLyrics);
      onDynamicThemeChange(netLyrics.dynamic_theme_enabled, netLyrics.weather_city);
      netLyricsFeedback = 'Настройки сохранены';
    } catch (e) {
      netLyricsFeedback = String(e);
    } finally {
      busy.netLyrics = false;
    }
  }

  $effect(() => {
    for (const s of sources) {
      if (!proxyDrafts[s.source]) {
        proxyDrafts[s.source] = { ...s.proxy };
      }
    }
  });

  function setProxyMode(source: SourceKind, mode: ProxyMode) {
    const cur = proxyDrafts[source] ?? { mode: 'direct', host: '127.0.0.1', port: 1080 };
    proxyDrafts[source] = {
      ...cur,
      mode,
      host: mode !== 'direct' && !cur.host ? '127.0.0.1' : cur.host,
      port: mode !== 'direct' && !cur.port ? 1080 : cur.port,
    };
  }

  async function saveProxy(source: SourceKind) {
    const cfg = proxyDrafts[source];
    if (!cfg) return;
    busy[`proxy-${source}`] = true;
    try {
      const next = await api.updateSourceProxy(source, cfg);
      onSourcesChanged(next);
      feedback[source] = 'Прокси сохранён';
    } catch (e) {
      feedback[source] = String(e);
    } finally {
      busy[`proxy-${source}`] = false;
    }
  }

  async function testApi(source: SourceKind) {
    busy[`test-${source}`] = true;
    try {
      const updated = await api.testSourceConnection(source);
      onSourcesChanged(sources.map((s) => (s.source === source ? updated : s)));
    } finally {
      busy[`test-${source}`] = false;
    }
  }

  async function handleConnectYandex() {
    busy.yandex = true;
    feedback.yandex = '';
    try {
      const next = await api.connectYandex(yandexToken);
      yandexToken = '';
      onSourcesChanged(next);
      feedback.yandex = 'Аккаунт Яндекс Музыки подключён';
    } catch (e) {
      feedback.yandex = String(e);
    } finally {
      busy.yandex = false;
    }
  }

  async function handleConnectSpotify() {
    busy.spotify = true;
    feedback.spotify = 'Откройте браузер и подтвердите вход в Spotify...';
    try {
      const next = await api.startSpotifyOAuth(spotifyClientId);
      onSourcesChanged(next);
      feedback.spotify = 'Spotify подключён';
    } catch (e) {
      feedback.spotify = String(e);
    } finally {
      busy.spotify = false;
    }
  }

  async function handleConnectSoundCloud() {
    busy.soundcloud = true;
    feedback.soundcloud = '';
    try {
      const next = await api.connectSoundCloud(scToken || null, scClientId || null);
      scToken = '';
      onSourcesChanged(next);
      feedback.soundcloud = 'SoundCloud подключён';
    } catch (e) {
      feedback.soundcloud = String(e);
    } finally {
      busy.soundcloud = false;
    }
  }

  async function handleDisconnect(source: SourceKind) {
    const next = await api.disconnectSource(source);
    onSourcesChanged(next);
    feedback[source] = 'Аккаунт отключён';
  }
</script>

<section class="settings-view">
  <div class="settings-head">
    <div>
      <h2>Настройки Flowly</h2>
      <p>Токены хранятся локально в зашищённом хранилище Rust-бэкенда и не передаются во frontend.</p>
    </div>
    <div class="settings-tabs">
      {#each settingsTabs as st (st.id)}
        <button
          type="button"
          class="s-tab-btn"
          class:active={settingsTab === st.id}
          onclick={() => (settingsTab = st.id)}
        >
          {st.label}
        </button>
      {/each}
    </div>
  </div>

  {#if settingsTab === 'services'}
  <!-- Выбор активных источников музыки -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Sparkles size={16} />
        <span class="net-title">Активные источники музыки</span>
      </div>
      <button
        type="button"
        class="btn-sm"
        class:active-pill={enabledSources.length === 3}
        onclick={() => onEnabledSourcesChange(['yandex', 'spotify', 'soundcloud'])}
      >
        Все сервисы
      </button>
    </div>
    <p class="active-src-desc">
      Выберите, какие площадки отображать в Волне, Библиотеке, Поиске и Плейлистах. Можно комбинировать или оставить только один сервис.
    </p>
    <div class="active-src-row">
      {#each sourceLabels as sl (sl.id)}
        {@const on = enabledSources.includes(sl.id)}
        <button
          type="button"
          class="src-toggle-pill"
          class:on
          onclick={() => toggleEnabledSource(sl.id)}
        >
          <SourceBadge source={sl.id} showLabel={false} />
          <span>{sl.label}</span>
          <span class="src-state">{on ? 'ВКЛ' : 'ВЫКЛ'}</span>
        </button>
        <button
          type="button"
          class="btn-sm"
          title="Оставить только {sl.label}"
          onclick={() => onEnabledSourcesChange([sl.id])}
        >
          Только
        </button>
      {/each}
    </div>
  </div>
  {/if}

  {#if settingsTab === 'appearance'}
  <!-- Режим закрытия окна -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Shield size={16} />
        <span class="net-title">Поведение при закрытии окна</span>
      </div>
      <button type="button" class="btn-sm" onclick={onOpenGuide}>
        <Sparkles size={12} />
        <span>Пройти обучение заново</span>
      </button>
    </div>
    <p class="active-src-desc">
      Выберите, что делать при нажатии на крестик: полностью завершать работу Flowly или сворачивать плеер в системный трей.
    </p>
    <div class="active-src-row">
      <button
        type="button"
        class="src-toggle-pill"
        class:on={!netLyrics.close_to_tray}
        onclick={() => saveNetLyrics({ close_to_tray: false })}
      >
        <span>Закрывать полностью</span>
      </button>
      <button
        type="button"
        class="src-toggle-pill"
        class:on={netLyrics.close_to_tray}
        onclick={() => saveNetLyrics({ close_to_tray: true })}
      >
        <span>Сворачивать в трей</span>
      </button>
    </div>
  </div>

  <!-- Динамическая тема по времени и погоде -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Sparkles size={16} />
        <span class="net-title">Динамическая тема (Время и погода)</span>
      </div>
      <button
        type="button"
        class="btn-sm"
        class:active-pill={netLyrics.dynamic_theme_enabled}
        onclick={() => saveNetLyrics({ dynamic_theme_enabled: !netLyrics.dynamic_theme_enabled })}
      >
        {netLyrics.dynamic_theme_enabled ? 'Тема: Включена' : 'Тема: Выключена'}
      </button>
    </div>
    <p class="active-src-desc">
      Акцент и атмосфера плеера автоматически подстраиваются под время суток (утро, день, вечер,
      ночь) и текущую погоду в указанном городе.
    </p>
    <div class="auth-box">
      <div class="auth-row">
        <input
          type="text"
          bind:value={netLyrics.weather_city}
          placeholder="Город (например: Москва или Казань)"
        />
        <button class="btn-accent" disabled={busy.netLyrics} onclick={() => saveNetLyrics()}>
          <RefreshCw size={13} />
          <span>Применить</span>
        </button>
      </div>
      {#if netLyrics.dynamic_theme_enabled && atmosphereSummary}
        <div class="feedback">Сейчас в эфире: {atmosphereSummary}</div>
      {/if}
    </div>
  </div>

  <!-- Режим экономии ОЗУ -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Cpu size={16} />
        <span class="net-title">Режим экономии ОЗУ и ресурсов</span>
      </div>
      <button
        type="button"
        class="btn-sm"
        class:active-pill={ramSaverEnabled}
        onclick={() => onRamSaverChange(!ramSaverEnabled)}
      >
        {ramSaverEnabled ? 'Экономия ОЗУ: Включена' : 'Экономия ОЗУ: Выключена'}
      </button>
    </div>
    <p class="active-src-desc">
      Отключает предзагрузку второго трека в память, немедленно очищает буфер неактивной аудио-деки и убирает фоновые анимации и погодные эффекты.
    </p>
  </div>

  <!-- OTA Обновления -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Download size={16} />
        <span class="net-title">Обновления Flowly (OTA)</span>
      </div>
      <div class="card-actions">
        {#if updateStatus === 'available'}
          <button type="button" class="btn-accent" onclick={installOtaUpdate}>
            <Download size={12} />
            <span>Установить v{updateVersion} и перезапустить</span>
          </button>
        {:else}
          <button
            type="button"
            class="btn-sm"
            disabled={updateStatus === 'checking' || updateStatus === 'installing'}
            onclick={checkForOtaUpdate}
          >
            <RefreshCw size={12} class={updateStatus === 'checking' ? 'spin' : ''} />
            <span>
              {updateStatus === 'checking'
                ? 'Проверка...'
                : updateStatus === 'installing'
                  ? 'Установка...'
                  : 'Проверить обновления'}
            </span>
          </button>
        {/if}
      </div>
    </div>
    <p class="active-src-desc">
      Автоматически загружает подписанные обновления из GitHub Releases (`whoisquickyyy/flowly`).
    </p>
    {#if updateStatus === 'latest'}
      <div class="feedback">У вас установлена последняя версия.</div>
    {:else if updateStatus === 'available'}
      <div class="feedback">Доступна новая версия v{updateVersion}{updateNotes ? `: ${updateNotes}` : ''}</div>
    {:else if updateStatus === 'error' && updateError}
      <div class="feedback err">{updateError}</div>
    {/if}
  </div>
  {/if}

  {#if settingsTab === 'integrations'}
  <!-- Детектор активности -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Shield size={16} />
        <span class="net-title">Детектор активности (Умные режимы)</span>
      </div>
      <button
        type="button"
        class="btn-sm"
        class:active-pill={activityConfig.enabled}
        onclick={() => onActivityConfigChange({ ...activityConfig, enabled: !activityConfig.enabled })}
      >
        {activityConfig.enabled ? 'Детектор: Включён' : 'Детектор: Выключен'}
      </button>
    </div>
    <p class="active-src-desc">
      Отслеживает активное окно (с задержкой 4 сек от случайных Alt-Tab): при кодинге включает
      плейлист и экономит GPU, в играх ставит Gaming EQ, а на YouTube/Twitch ставит паузу.
    </p>

    {#if activityConfig.enabled && currentActivityLabel}
      <div class="feedback">Сейчас активно: {currentActivityLabel}</div>
    {/if}

    <div class="act-rules">
      <div class="act-rule-row">
        <span class="act-lbl">Кодинг (VS Code, Cursor, JetBrains):</span>
        <select
          class="act-select"
          value={activityConfig.codingPlaylistId}
          onchange={(e) =>
            onActivityConfigChange({ ...activityConfig, codingPlaylistId: e.currentTarget.value })}
        >
          <option value="">Плейлист: не менять</option>
          {#each playlists as pl (pl.id)}
            <option value={pl.id}>{pl.title}</option>
          {/each}
        </select>
        <select
          class="act-select"
          value={activityConfig.codingEqPreset}
          onchange={(e) =>
            onActivityConfigChange({ ...activityConfig, codingEqPreset: e.currentTarget.value })}
        >
          <option value="">EQ: не менять</option>
          <option value="Lo-Fi">EQ: Lo-Fi</option>
          <option value="Flat">EQ: Flat</option>
          <option value="Late Night">EQ: Late Night</option>
        </select>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={activityConfig.codingEco}
          onclick={() => onActivityConfigChange({ ...activityConfig, codingEco: !activityConfig.codingEco })}
        >
          Eco GPU: {activityConfig.codingEco ? 'Вкл' : 'Выкл'}
        </button>
      </div>

      <div class="act-rule-row">
        <span class="act-lbl">Игры (Steam, Epic, Riot + свои):</span>
        <select
          class="act-select"
          value={activityConfig.gamingEqPreset}
          onchange={(e) =>
            onActivityConfigChange({ ...activityConfig, gamingEqPreset: e.currentTarget.value })}
        >
          <option value="">EQ: не менять</option>
          <option value="Gaming">EQ: Gaming</option>
          <option value="Bass Boost">EQ: Bass Boost</option>
          <option value="Electronic">EQ: Electronic</option>
        </select>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={activityConfig.gamingEco}
          onclick={() => onActivityConfigChange({ ...activityConfig, gamingEco: !activityConfig.gamingEco })}
        >
          Eco GPU: {activityConfig.gamingEco ? 'Вкл' : 'Выкл'}
        </button>
      </div>

      <div class="act-rule-row">
        <span class="act-lbl">Свои процессы игр (через запятую):</span>
        <input
          class="act-input"
          type="text"
          value={activityConfig.customGames}
          placeholder="cs2, minecraft, gta5..."
          oninput={(e) =>
            onActivityConfigChange({ ...activityConfig, customGames: (e.target as HTMLInputElement).value })}
        />
      </div>

      <div class="act-rule-row">
        <span class="act-lbl">Видео (YouTube, Twitch, Кинопоиск):</span>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={activityConfig.videoAutoPause}
          onclick={() =>
            onActivityConfigChange({ ...activityConfig, videoAutoPause: !activityConfig.videoAutoPause })}
        >
          Авто-пауза: {activityConfig.videoAutoPause ? 'Вкл' : 'Выкл'}
        </button>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={activityConfig.videoAutoResume}
          onclick={() =>
            onActivityConfigChange({ ...activityConfig, videoAutoResume: !activityConfig.videoAutoResume })}
        >
          Авто-возобновление: {activityConfig.videoAutoResume ? 'Вкл' : 'Выкл'}
        </button>
      </div>
    </div>
  </div>
  {/if}

  {#if settingsTab === 'services'}
  <div class="services-list">
    {#each sources as src (src.source)}
      {@const draft = proxyDrafts[src.source] ?? src.proxy}
      <div class="service-card">
        <div class="service-top">
          <div class="s-identity">
            <SourceBadge source={src.source} />
            {#if src.auth.connected}
              <span class="badge ok">
                <CheckCircle2 size={12} />
                <span>Подключен{src.auth.account_name ? `: ${src.auth.account_name}` : ''}</span>
              </span>
            {:else}
              <span class="badge off">
                <XCircle size={12} />
                <span>Не подключен</span>
              </span>
            {/if}
          </div>

          <div class="top-btns">
            <button
              class="btn-sm"
              disabled={busy[`test-${src.source}`]}
              onclick={() => testApi(src.source)}
            >
              <RefreshCw size={12} />
              <span>
                {src.is_reachable === true
                  ? `API OK (${src.latency_ms} мс)`
                  : src.is_reachable === false
                  ? 'API недоступен'
                  : 'Проверить API'}
              </span>
            </button>

            {#if src.auth.connected}
              <button class="btn-sm danger" onclick={() => handleDisconnect(src.source)}>
                <LogOut size={12} />
                <span>Отключить</span>
              </button>
            {/if}
          </div>
        </div>

        <!-- Авторизация -->
        <div class="auth-block">
          {#if src.source === 'yandex'}
            <div class="guide-box">
              <strong>Как подключить Yandex Music:</strong>
              <ol>
                <li>Нажмите <b>«Открыть вход Яндекс»</b> — в браузере откроется страница авторизации.</li>
                <li>Войдите в свой аккаунт Яндекса и разрешите доступ.</li>
                <li>Когда страница перенаправит вас, скопируйте <b>всю ссылку из адресной строки</b> (или только <code>access_token</code>) и вставьте ниже.</li>
              </ol>
            </div>
            <div class="auth-row">
              <input
                type="password"
                bind:value={yandexToken}
                placeholder="Вставьте скопированную ссылку или OAuth-токен..."
              />
              <button
                class="btn-sm"
                onclick={() =>
                  api.openExternalUrl(
                    'https://oauth.yandex.ru/authorize?response_type=token&client_id=23cabbbdc6cd418abb4b39c32c41195d'
                  )}
              >
                <ExternalLink size={12} />
                <span>Открыть вход Яндекс</span>
              </button>
              <button class="btn-accent" disabled={busy.yandex} onclick={handleConnectYandex}>
                <KeyRound size={13} />
                <span>{busy.yandex ? 'Проверка...' : 'Подключить'}</span>
              </button>
            </div>
          {:else if src.source === 'spotify'}
            <div class="guide-box">
              <strong>Как подключить Spotify (OAuth 2.0 PKCE):</strong>
              <ol>
                <li>Если работает системный VPN (как и для десктопного Spotify) — выберите сеть <b>Direct</b>. Если используете локальный прокси (v2rayN/Nekoray) — укажите <b>SOCKS5/HTTP</b> и нажмите «Применить прокси».</li>
                <li>В <button class="inline-link" onclick={() => api.openExternalUrl('https://developer.spotify.com/dashboard')}>Spotify Developer Dashboard</button> создайте приложение, добавьте Redirect URI: <code>http://127.0.0.1:8888/callback</code> и в разделе <b>User Management</b> добавьте свой email Spotify (иначе будет ошибка 403).</li>
                <li>Скопируйте <b>Client ID</b> в поле ниже и нажмите <b>«Войти через OAuth PKCE»</b>.</li>
              </ol>
            </div>
            <div class="auth-row">
              <input
                type="text"
                bind:value={spotifyClientId}
                placeholder="Вставьте Spotify Client ID..."
              />
              <button class="btn-accent" disabled={busy.spotify} onclick={handleConnectSpotify}>
                <KeyRound size={13} />
                <span>{busy.spotify ? 'Ожидание браузера...' : 'Войти через OAuth PKCE'}</span>
              </button>
            </div>
          {:else}
            <div class="guide-box">
              <strong>Как подключить SoundCloud:</strong>
              <ol>
                <li>В РФ обычно требуется <b>SOCKS5/HTTP прокси</b> — настройте его в блоке «Сеть» ниже.</li>
                <li>Укажите <b>Client ID</b> (для поиска и публичных треков) и/или <b>OAuth Token</b> (для вашей медиатеки).</li>
              </ol>
            </div>
            <div class="auth-row">
              <input type="password" bind:value={scToken} placeholder="OAuth Token (опционально)" />
              <input type="text" bind:value={scClientId} placeholder="Client ID" />
              <button
                class="btn-accent"
                disabled={busy.soundcloud}
                onclick={handleConnectSoundCloud}
              >
                <KeyRound size={13} />
                <span>Подключить</span>
              </button>
            </div>
          {/if}

          {#if feedback[src.source]}
            <div class="feedback">{feedback[src.source]}</div>
          {/if}
          {#if src.last_error}
            <div class="feedback err">{src.last_error}</div>
          {/if}
        </div>

        <!-- Прокси для РФ (Split-Tunneling) -->
        <div class="proxy-row">
          <span class="row-lbl">
            <Shield size={13} />
            <span>Сеть:</span>
          </span>
          <div class="mode-pills">
            <button class:active={draft.mode === 'direct'} onclick={() => setProxyMode(src.source, 'direct')}>Direct</button>
            <button class:active={draft.mode === 'socks5'} onclick={() => setProxyMode(src.source, 'socks5')}>SOCKS5</button>
            <button class:active={draft.mode === 'http'} onclick={() => setProxyMode(src.source, 'http')}>HTTP</button>
          </div>
          {#if draft.mode !== 'direct'}
            <input class="p-host" type="text" bind:value={draft.host} placeholder="127.0.0.1" />
            <input class="p-port" type="number" bind:value={draft.port} placeholder="1080" />
          {/if}
          <button class="btn-sm" onclick={() => saveProxy(src.source)}>Применить прокси</button>
        </div>

        <!-- Provider Capability System -->
        <div class="caps-row">
          <span class="cap" class:yes={src.auth.capabilities.can_search}>Поиск</span>
          <span class="cap" class:yes={src.auth.capabilities.can_stream_in_app}>Стриминг в плеере</span>
          <span class="cap" class:yes={src.auth.capabilities.can_remote_control}>Remote Control</span>
          <span class="cap" class:yes={src.auth.capabilities.can_read_library}>Библиотека</span>
          <span class="cap" class:yes={src.auth.capabilities.can_read_playlists}>Плейлисты</span>
        </div>
      </div>
    {/each}
  </div>
  {/if}

  {#if settingsTab === 'network'}
  <!-- Настройки DoH и Spicy Lyrics -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Globe size={16} />
        <span class="net-title">DNS-over-HTTPS (DoH)</span>
      </div>
      <div class="card-actions">
        <button
          type="button"
          class="btn-sm"
          disabled={checkingAllDoh}
          onclick={checkAllDohServers}
        >
          <RefreshCw size={12} class={checkingAllDoh ? 'spin' : ''} />
          <span>{checkingAllDoh ? 'Проверка...' : 'Проверить все'}</span>
        </button>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={netLyrics.doh_enabled}
          onclick={() => saveNetLyrics({ doh_enabled: !netLyrics.doh_enabled })}
        >
          {netLyrics.doh_enabled ? 'DoH: Включён' : 'DoH: Выключен'}
        </button>
      </div>
    </div>

    <div class="doh-group-lbl">Без подмены (только шифрование)</div>
    <div class="doh-grid">
      {#each dohPlainServers as srv (srv.id)}
        {@const lat = dohLatencies[srv.id]}
        <button
          type="button"
          class="doh-card"
          class:selected={netLyrics.doh_enabled && netLyrics.doh_provider === srv.id}
          onclick={() => saveNetLyrics({ doh_provider: srv.id, doh_enabled: true })}
        >
          <div class="doh-card-top">
            <span class="doh-name">{srv.name}</span>
            {#if lat?.ms !== undefined}
              <span class="doh-ms">{lat.ms} мс</span>
            {:else if lat?.err}
              <span class="doh-err">таймаут</span>
            {/if}
          </div>
          <span class="doh-url">{srv.url}</span>
        </button>
      {/each}
    </div>

    <div class="doh-group-lbl">С обходом геоблокировок</div>
    <div class="doh-grid">
      {#each dohGeoServers as srv (srv.id)}
        {@const lat = dohLatencies[srv.id]}
        <button
          type="button"
          class="doh-card"
          class:selected={netLyrics.doh_enabled && netLyrics.doh_provider === srv.id}
          onclick={() => saveNetLyrics({ doh_provider: srv.id, doh_enabled: true })}
        >
          <div class="doh-card-top">
            <span class="doh-name">{srv.name}</span>
            {#if lat?.ms !== undefined}
              <span class="doh-ms">{lat.ms} мс</span>
            {:else if lat?.err}
              <span class="doh-err">таймаут</span>
            {/if}
          </div>
          <span class="doh-url">{srv.url}</span>
        </button>
      {/each}
    </div>

    <div class="auth-box">
      {#if netLyricsFeedback}
        <div class="feedback">{netLyricsFeedback}</div>
      {/if}
    </div>
  </div>
  {/if}

  {#if settingsTab === 'integrations'}
  <!-- Spicy Lyrics -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Sparkles size={16} />
        <span class="net-title">Spicy Lyrics (Синхронизированные тексты)</span>
      </div>
    </div>
    <p class="active-src-desc">
      Подключите API-ключ Spicy Lyrics для расширенных построчных текстов и караоке-подсветки.
    </p>
    <div class="auth-box">
      <div class="auth-row">
        <input
          type="password"
          bind:value={netLyrics.spicy_lyrics_key}
          placeholder="Свой API-ключ (опционально, по умолчанию встроенный Flowly)"
        />
        <button class="btn-accent" disabled={busy.netLyrics} onclick={() => saveNetLyrics()}>
          <KeyRound size={13} />
          <span>Сохранить</span>
        </button>
        <button
          type="button"
          class="btn-sm"
          onclick={() => api.openExternalUrl('https://developers.spicylyrics.org/')}
        >
          <ExternalLink size={12} />
          <span>Получить ключ</span>
        </button>
      </div>
      {#if netLyricsFeedback}
        <div class="feedback">{netLyricsFeedback}</div>
      {/if}
    </div>
  </div>

  <!-- Симуляция радио (Виртуальный ведущий) -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Mic size={16} />
        <span class="net-title">Симуляция радио (Виртуальный ведущий)</span>
      </div>
      <div class="card-actions">
        {#if radio.engine === 'piper'}
          <button
            type="button"
            class="btn-sm"
            disabled={installingPiper}
            onclick={handleInstallPiper}
          >
            <Download size={12} />
            <span>{installingPiper ? 'Установка...' : 'Установить Piper'}</span>
          </button>
        {/if}
        <button type="button" class="btn-sm" onclick={onTestRadio}>
          Тест голоса
        </button>
        <button
          type="button"
          class="btn-sm"
          class:active-pill={radio.enabled}
          onclick={() => onRadioChange({ ...radio, enabled: !radio.enabled })}
        >
          {radio.enabled ? 'Ведущий: ВКЛ' : 'Ведущий: ВЫКЛ'}
        </button>
      </div>
    </div>

    <p class="active-src-desc">
      Объявляет треки и погоду в эфире, когда трек заканчивается сам (не срабатывает при ручном перелистывании).
    </p>

    <div class="radio-grid">
      <div class="radio-group">
        <span class="rg-lbl">Движок озвучки</span>
        <div class="mode-pills">
          <button type="button" class:active={radio.engine === 'edge'} onclick={() => onRadioChange({ ...radio, engine: 'edge' })}>Edge Neural</button>
          <button type="button" class:active={radio.engine === 'piper'} onclick={() => onRadioChange({ ...radio, engine: 'piper' })}>Piper (локально)</button>
          <button type="button" class:active={radio.engine === 'system'} onclick={() => onRadioChange({ ...radio, engine: 'system' })}>Системный</button>
        </div>
      </div>

      <div class="radio-group">
        <span class="rg-lbl">Голос</span>
        <div class="mode-pills">
          <button type="button" class:active={radio.voice === 'male'} onclick={() => onRadioChange({ ...radio, voice: 'male' })}>Дмитрий</button>
          <button type="button" class:active={radio.voice === 'female'} onclick={() => onRadioChange({ ...radio, voice: 'female' })}>Светлана</button>
        </div>
      </div>

      <div class="radio-group">
        <span class="rg-lbl">Частота выхода</span>
        <div class="mode-pills">
          <button type="button" class:active={radio.frequency === 2} onclick={() => onRadioChange({ ...radio, frequency: 2 })}>Раз в 2</button>
          <button type="button" class:active={radio.frequency === 3} onclick={() => onRadioChange({ ...radio, frequency: 3 })}>Раз в 3</button>
          <button type="button" class:active={radio.frequency === 5} onclick={() => onRadioChange({ ...radio, frequency: 5 })}>Раз в 5</button>
        </div>
      </div>

      <div class="radio-group">
        <span class="rg-lbl">Город (для погоды в эфире)</span>
        <input
          type="text"
          class="radio-city-input"
          value={radio.city}
          placeholder="Например: Москва"
          oninput={(e) => onRadioChange({ ...radio, city: (e.target as HTMLInputElement).value })}
        />
      </div>
    </div>
    {#if piperInstallMsg}
      <div class="feedback">{piperInstallMsg}</div>
    {/if}
    {#if radioStatus}
      <div class="feedback">{radioStatus}</div>
    {/if}
  </div>

  <!-- Discord Rich Presence -->
  <div class="service-card">
    <div class="card-top">
      <div class="card-title">
        <Sparkles size={16} />
        <span class="net-title">Discord Rich Presence (RPC)</span>
      </div>
      <button
        type="button"
        class="btn-sm"
        class:active-pill={netLyrics.discord_rpc_enabled}
        onclick={() => saveNetLyrics({ discord_rpc_enabled: !netLyrics.discord_rpc_enabled })}
      >
        {netLyrics.discord_rpc_enabled ? 'Discord RPC: Включён' : 'Discord RPC: Выключен'}
      </button>
    </div>

    <p class="active-src-desc">
      Работает из коробки и показывает в профиле Discord текущий трек, исполнителя, обложку и таймер прослушивания. При желании можно указать свой Application ID.
    </p>

    <div class="auth-box">
      <div class="auth-row">
        <input
          type="text"
          bind:value={netLyrics.discord_client_id}
          placeholder="Свой Application ID (опционально, по умолчанию встроенный Flowly)"
        />
        <button class="btn-accent" disabled={busy.netLyrics} onclick={() => saveNetLyrics()}>
          <KeyRound size={13} />
          <span>Сохранить</span>
        </button>
        <button
          type="button"
          class="btn-sm"
          onclick={() => api.openExternalUrl('https://discord.com/developers/applications')}
        >
          <ExternalLink size={12} />
          <span>Создать App</span>
        </button>
      </div>
    </div>
  </div>
  {/if}

  {#if settingsTab === 'themes'}
  <div class="themes-section">
    <div class="service-card">
      <div class="card-top">
        <div class="card-title">
          <Sparkles size={16} />
          <span class="net-title">Магазин CSS-тем</span>
        </div>
        <span class="theme-count-badge">{cssThemeStore.length} тем</span>
      </div>
      <p class="active-src-desc">
        Выберите готовую тему или загрузите её в редактор ниже для доработки под себя.
      </p>

      <div class="theme-store-grid">
        {#each cssThemeStore as theme (theme.id)}
          {@const active = activeCssThemeId === theme.id}
          <div class="theme-card" class:active>
            <div class="theme-card-top">
              <div class="theme-swatches">
                {#each theme.swatches as col}
                  <span class="swatch-dot" style="background: {col}"></span>
                {/each}
              </div>
              {#if active}
                <span class="theme-active-badge">Активна</span>
              {/if}
            </div>
            <div class="theme-info">
              <div class="theme-name">{theme.name}</div>
              <div class="theme-author">by {theme.author}</div>
              <div class="theme-desc">{theme.desc}</div>
            </div>
            <div class="theme-actions">
              <button
                type="button"
                class={active ? 'btn-accent' : 'btn-sm'}
                onclick={() => {
                  cssDraft = theme.css;
                  onApplyCssTheme(theme.id, theme.css);
                }}
              >
                {active ? 'Применена' : 'Применить'}
              </button>
              <button
                type="button"
                class="btn-sm"
                onclick={() => {
                  cssDraft = theme.css;
                }}
              >
                В редактор
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="service-card">
      <div class="card-top">
        <div class="card-title">
          <KeyRound size={16} />
          <span class="net-title">Редактор своей CSS-темы</span>
        </div>
        <span class="css-len">{cssDraft.length} симв.</span>
      </div>
      <p class="active-src-desc">
        Пишите любой CSS поверх интерфейса. Основные переменные: <code>--accent</code>, <code>--bg-graphite</code>, <code>--bg-surface</code>, <code>--bg-elevated</code>.
      </p>

      <div class="css-snippets">
        <button type="button" class="btn-sm" onclick={() => insertCssSnippet('--accent: #00e5ff;')}>+ --accent</button>
        <button type="button" class="btn-sm" onclick={() => insertCssSnippet('--bg-graphite: #0a0f18;')}>+ --bg-graphite</button>
        <button type="button" class="btn-sm" onclick={() => insertCssSnippet('--bg-surface: #111827;')}>+ --bg-surface</button>
        <button type="button" class="btn-sm" onclick={() => insertCssSnippet('--radius-md: 16px;')}>+ --radius-md</button>
      </div>

      <textarea
        class="css-editor"
        bind:value={cssDraft}
        spellcheck={false}
        placeholder={`:root {\n  --accent: #a855f7;\n  --bg-graphite: #0c0914;\n  --bg-surface: #151024;\n}\n\n.artwork-img {\n  border-radius: 24px;\n}`}
      ></textarea>

      <div class="css-editor-actions">
        <button
          type="button"
          class="btn-accent"
          onclick={() => onApplyCssTheme('custom', cssDraft)}
        >
          Применить свой CSS
        </button>
        <button
          type="button"
          class="btn-sm"
          onclick={async () => {
            try {
              await navigator.clipboard.writeText(cssDraft);
              cssCopied = true;
              setTimeout(() => (cssCopied = false), 1600);
            } catch {}
          }}
        >
          {cssCopied ? 'Скопировано!' : 'Копировать CSS'}
        </button>
        <button
          type="button"
          class="btn-sm"
          onclick={() => {
            cssDraft = '';
            onApplyCssTheme('default', '');
          }}
        >
          Сбросить к стандартной
        </button>
      </div>
    </div>
  </div>
  {/if}
</section>

<style>
  .settings-view {
    padding: 20px 28px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .settings-head {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .settings-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .s-tab-btn {
    padding: 7px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.14s ease, color 0.14s ease, border-color 0.14s ease;
  }

  .s-tab-btn:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .s-tab-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #0b0b0e;
    font-weight: 700;
  }

  .settings-head h2 {
    font-size: 20px;
    font-weight: 700;
  }

  .settings-head p {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 4px;
  }

  .services-list {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .service-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .service-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .s-identity {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    padding: 3px 9px;
    border-radius: 999px;
  }

  .badge.ok {
    background: rgba(16, 185, 129, 0.14);
    color: #34d399;
  }

  .badge.off {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
  }

  .top-btns {
    display: flex;
    gap: 8px;
  }

  .auth-block {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .guide-box {
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--bg-graphite);
    border: 1px solid var(--border-subtle);
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .guide-box strong {
    display: block;
    color: var(--text-primary);
    margin-bottom: 4px;
    font-weight: 600;
  }

  .guide-box ol {
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .guide-box code {
    font-family: ui-monospace, monospace;
    font-size: 11px;
    background: var(--bg-elevated);
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--accent);
  }

  .inline-link {
    border: none;
    background: transparent;
    color: var(--accent);
    font-size: inherit;
    cursor: pointer;
    text-decoration: underline;
    padding: 0;
  }

  .auth-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .auth-row input {
    flex: 1;
    min-width: 200px;
    padding: 7px 11px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: var(--bg-graphite);
    color: var(--text-primary);
    font-size: 12px;
  }

  .btn-accent {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #111215;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .btn-sm {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 11px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn-sm:hover {
    background: var(--bg-hover);
  }

  .btn-sm.danger:hover {
    border-color: rgba(244, 63, 94, 0.5);
    color: #fb7185;
  }

  .proxy-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding-top: 8px;
    border-top: 1px solid var(--border-subtle);
  }

  .row-lbl {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .mode-pills {
    display: flex;
    gap: 4px;
  }

  .mode-pills button {
    padding: 4px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
  }

  .mode-pills button.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border-color: var(--border-strong);
  }

  .p-host,
  .p-port {
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: var(--bg-graphite);
    color: var(--text-primary);
    font-size: 12px;
  }

  .p-host {
    width: 120px;
  }

  .p-port {
    width: 72px;
  }

  .caps-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .cap {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .cap.yes {
    color: #34d399;
    background: rgba(16, 185, 129, 0.08);
    border-color: rgba(16, 185, 129, 0.25);
  }

  .feedback {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .feedback.err {
    color: #fb7185;
  }

  .card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .card-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .net-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .btn-sm.active-pill {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--text-primary);
  }

  .doh-group-lbl {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .doh-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .doh-card {
    text-align: left;
    padding: 9px 11px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.025);
    display: flex;
    flex-direction: column;
    gap: 3px;
    cursor: pointer;
    transition: border-color 0.14s ease, background-color 0.14s ease, transform 0.12s ease;
  }

  .doh-card:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--border-strong);
  }

  .doh-card.selected {
    border-color: var(--text-primary);
    background: rgba(255, 255, 255, 0.08);
    box-shadow: 0 0 18px rgba(255, 255, 255, 0.06);
  }

  .doh-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .doh-name {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .doh-url {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .doh-ms {
    font-size: 10px;
    font-weight: 700;
    color: #34d399;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .doh-err {
    font-size: 10px;
    color: #fb7185;
    flex-shrink: 0;
  }

  .active-src-desc {
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .act-rules {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .act-rule-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .act-lbl {
    font-size: 12px;
    color: var(--text-secondary);
    min-width: 220px;
  }

  .act-select,
  .act-input {
    padding: 6px 10px;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.7);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
  }

  .act-input {
    flex: 1;
    min-width: 180px;
  }

  .active-src-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .src-toggle-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.025);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.14s ease;
  }

  .src-toggle-pill.on {
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .src-state {
    font-size: 10px;
    font-weight: 700;
    opacity: 0.8;
  }

  .themes-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .theme-store-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: 14px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.025);
    transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease;
  }

  .theme-card:hover {
    border-color: rgba(255, 255, 255, 0.16);
    transform: translateY(-1px);
  }

  .theme-card.active {
    border-color: color-mix(in srgb, var(--accent) 65%, transparent);
    background: color-mix(in srgb, var(--accent) 11%, rgba(255, 255, 255, 0.02));
  }

  .theme-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .theme-active-badge,
  .theme-count-badge {
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .theme-count-badge {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    border-color: var(--border-subtle);
  }

  .theme-swatches {
    display: flex;
    gap: 6px;
  }

  .swatch-dot {
    width: 20px;
    height: 20px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.35);
  }

  .theme-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }

  .theme-name {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .theme-author {
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .theme-desc {
    font-size: 11.5px;
    line-height: 1.38;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .theme-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .css-len {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .css-snippets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .css-editor {
    width: 100%;
    min-height: 210px;
    resize: vertical;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid var(--border-subtle);
    background: rgba(8, 8, 12, 0.82);
    color: var(--text-primary);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.5;
    outline: none;
  }

  .css-editor:focus {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
  }

  .css-editor-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .radio-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 12px;
  }

  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rg-lbl {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .radio-city-input {
    padding: 7px 11px;
    border-radius: 9px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.7);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
  }
</style>
