<script lang="ts">
  import {
    ArrowLeft,
    ArrowRight,
    BarChart3,
    Disc3,
    Globe,
    Heart,
    Palette,
    Play,
    Radio,
    Search,
    Sliders,
    Sparkles,
    X,
  } from 'lucide-svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
    onGoSettings?: () => void;
  }

  let { open, onClose, onGoSettings }: Props = $props();
  let step = $state(0);

  const totalSteps = 5;

  function finish() {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_onboarding_done', '1');
    }
    step = 0;
    onClose();
  }

  function next() {
    if (step < totalSteps - 1) {
      step += 1;
    } else {
      finish();
    }
  }

  function prev() {
    if (step > 0) step -= 1;
  }
</script>

{#if open}
  <div
    class="ob-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) finish();
    }}
  >
    <div class="ob-modal">
      <div class="ob-top">
        <span class="ob-step-pill">Шаг {step + 1} из {totalSteps}</span>
        <button type="button" class="ob-skip" onclick={finish}>
          <span>Пропустить</span>
          <X size={14} />
        </button>
      </div>

      <!-- Визуальная мини-сцена (интерактивный макет интерфейса) -->
      <div class="ob-stage">
        {#if step === 0}
          <div class="mock-player">
            <div class="mock-glow"></div>
            <div class="mock-cover">
              <Disc3 size={38} />
            </div>
            <div class="mock-meta">
              <div class="mock-title">Flowly Player</div>
              <div class="mock-sub">Yandex Music · Spotify · SoundCloud</div>
            </div>
            <div class="mock-bar">
              <Heart size={13} fill="#ff3b5c" color="#ff3b5c" />
              <div class="mock-line"><span></span></div>
              <div class="mock-play"><Play size={12} fill="currentColor" /></div>
            </div>
          </div>
        {:else if step === 1}
          <div class="mock-card">
            <div class="mock-row-head">
              <Globe size={15} />
              <span>Сервисы и сеть (DoH)</span>
            </div>
            <div class="mock-pills">
              <span class="mp ok">Яндекс Музыка · Подключено</span>
              <span class="mp ok">Spotify · Подключено</span>
              <span class="mp">SoundCloud · Прокси / Direct</span>
            </div>
            <div class="mock-hint-box">
              Встроенный DoH и прокси работают без системного VPN
            </div>
          </div>
        {:else if step === 2}
          <div class="mock-card">
            <div class="mock-search-bar">
              <Search size={13} />
              <span>«дефт панк» → <b>Daft Punk</b></span>
            </div>
            <div class="mock-wave-box">
              <Sparkles size={15} />
              <div>
                <div class="mw-title">Моя волна · Бесконечный поток</div>
                <div class="mw-sub">Микс · Энергия · Чилл · Открытия</div>
              </div>
            </div>
          </div>
        {:else if step === 3}
          <div class="mock-card">
            <div class="mock-row-head">
              <Sliders size={15} />
              <span>10-полосный EQ & DJ-переходы</span>
              <Palette size={14} style="margin-left: auto" />
            </div>
            <div class="mock-eq-bars">
              <span style="height: 55%"></span>
              <span style="height: 75%"></span>
              <span style="height: 45%"></span>
              <span style="height: 35%"></span>
              <span style="height: 60%"></span>
              <span style="height: 82%"></span>
              <span style="height: 68%"></span>
              <span style="height: 50%"></span>
            </div>
            <div class="mock-pills">
              <span class="mp ok">Beat + Vocal</span>
              <span class="mp">Дождь на стекле</span>
              <span class="mp">Живая типографика</span>
            </div>
          </div>
        {:else}
          <div class="mock-card">
            <div class="mock-row-head">
              <Radio size={15} />
              <span>Комната FLOW-4829</span>
              <BarChart3 size={14} style="margin-left: auto" />
            </div>
            <div class="mock-members">
              <div class="mm-pill">
                <span class="mm-av">К</span>
                <span>Квики</span>
                <span class="mm-role">Создатель</span>
              </div>
              <div class="mm-pill">
                <span class="mm-av">А</span>
                <span>Алекс</span>
                <span class="mm-sub">Совпадение · 14 тр.</span>
              </div>
            </div>
            <div class="mock-hint-box">
              Совмещение вкусов друзей + Итоги дня, недели и месяца
            </div>
          </div>
        {/if}
      </div>

      <!-- Текст шага -->
      <div class="ob-body">
        {#if step === 0}
          <h3>Добро пожаловать во Flowly</h3>
          <p>
            Единый плеер для Яндекс Музыки, Spotify и SoundCloud с красивыми фонами, DJ-кроссфейдами,
            Discord RPC и совместным прослушиванием с друзьями.
          </p>
        {:else if step === 1}
          <h3>1. Подключи аккаунты и сеть</h3>
          <p>
            Открой <b>Настройки</b> (иконка шестерёнки слева внизу) и подключи свои сервисы. Если
            какой-то сервис не открывается, там же включается <b>DoH</b> или локальный <b>SOCKS5/HTTP прокси</b>.
          </p>
        {:else if step === 2}
          <h3>2. «Моя волна», Умный поиск и Сэмплы</h3>
          <p>
            Запускай бесконечную <b>«Мою волну»</b> с выбором настроения, ищи треки даже с опечатками
            и на русском («дефт панк» найдёт Daft Punk), а в меню <b>«...»</b> можно узнать сэмплы трека.
          </p>
        {:else if step === 3}
          <h3>3. Эквалайзер, DJ-микс и живые фоны</h3>
          <p>
            Настраивай звук во вкладке <b>Эквалайзер</b> (10 полос, Bass Boost, Auto-Mix переходы),
            а в правом верхнем углу нажми на <b>Палитру</b>, чтобы выбрать Ambient Light, Космос,
            Дождь или Живую типографику.
          </p>
        {:else}
          <h3>4. Профиль, Итоги и Комнаты (Jam)</h3>
          <p>
            Создай аккаунт Flowly (иконка профиля слева), загрузи и обрезь аватарку. Включай вкладку
            <b>«Итоги»</b> для анализа вкуса и заходи в <b>«Комнату»</b>, чтобы слушать общий микс с друзьями!
          </p>
        {/if}
      </div>

      <!-- Навигация -->
      <div class="ob-footer">
        <div class="ob-dots">
          {#each Array(totalSteps) as _, i (i)}
            <button
              type="button"
              class="dot"
              class:active={i === step}
              onclick={() => (step = i)}
              aria-label="Шаг {i + 1}"
            ></button>
          {/each}
        </div>

        <div class="ob-btns">
          {#if step > 0}
            <button type="button" class="ob-btn ghost" onclick={prev}>
              <ArrowLeft size={14} />
              <span>Назад</span>
            </button>
          {/if}
          {#if step === 1 && onGoSettings}
            <button
              type="button"
              class="ob-btn ghost"
              onclick={() => {
                finish();
                onGoSettings();
              }}
            >
              <span>Открыть настройки</span>
            </button>
          {/if}
          <button type="button" class="ob-btn primary" onclick={next}>
            <span>{step === totalSteps - 1 ? 'Начать слушать' : 'Далее'}</span>
            <ArrowRight size={14} />
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .ob-backdrop {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: rgba(5, 5, 9, 0.78);
    backdrop-filter: blur(14px);
    display: grid;
    place-items: center;
    padding: 20px;
  }

  .ob-modal {
    width: 100%;
    max-width: 540px;
    padding: 22px 24px;
    border-radius: 22px;
    background: linear-gradient(160deg, rgba(24, 24, 34, 0.98), rgba(14, 14, 20, 0.98));
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 28px 70px rgba(0, 0, 0, 0.75);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .ob-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .ob-step-pill {
    font-size: 11px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }

  .ob-skip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
  }

  .ob-skip:hover {
    color: var(--text-primary);
  }

  .ob-stage {
    height: 190px;
    border-radius: 16px;
    background: rgba(8, 8, 13, 0.78);
    border: 1px solid var(--border-subtle);
    display: grid;
    place-items: center;
    padding: 16px;
    position: relative;
    overflow: hidden;
  }

  .mock-player {
    position: relative;
    width: 100%;
    max-width: 340px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .mock-glow {
    position: absolute;
    width: 180px;
    height: 180px;
    border-radius: 50%;
    background: radial-gradient(circle, color-mix(in srgb, var(--accent) 42%, transparent), transparent 70%);
    filter: blur(22px);
    pointer-events: none;
  }

  .mock-cover {
    position: relative;
    width: 72px;
    height: 72px;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.14);
    display: grid;
    place-items: center;
    color: var(--accent);
  }

  .mock-meta {
    text-align: center;
  }

  .mock-title {
    font-size: 15px;
    font-weight: 800;
  }

  .mock-sub {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .mock-bar {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mock-line {
    flex: 1;
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }

  .mock-line span {
    display: block;
    width: 58%;
    height: 100%;
    background: var(--accent);
  }

  .mock-play {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent);
    color: #0b0b0e;
    display: grid;
    place-items: center;
  }

  .mock-card {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .mock-row-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 700;
    color: var(--accent);
  }

  .mock-row-head span {
    color: var(--text-primary);
  }

  .mock-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .mp {
    font-size: 11px;
    padding: 5px 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
  }

  .mp.ok {
    background: rgba(16, 185, 129, 0.16);
    color: #34d399;
    font-weight: 600;
  }

  .mock-hint-box {
    font-size: 11.5px;
    padding: 8px 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
  }

  .mock-search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    font-size: 12px;
    color: var(--text-secondary);
  }

  .mock-search-bar b {
    color: var(--accent);
  }

  .mock-wave-box {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 14px;
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 22%, transparent), rgba(255, 255, 255, 0.03));
    border: 1px solid color-mix(in srgb, var(--accent) 38%, transparent);
    color: var(--accent);
  }

  .mw-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .mw-sub {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .mock-eq-bars {
    height: 54px;
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 0 8px;
  }

  .mock-eq-bars span {
    flex: 1;
    border-radius: 4px;
    background: linear-gradient(to top, color-mix(in srgb, var(--accent) 45%, transparent), var(--accent));
  }

  .mock-members {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .mm-pill {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.05);
    font-size: 12px;
    font-weight: 600;
  }

  .mm-av {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 11px;
    font-weight: 800;
    display: grid;
    place-items: center;
  }

  .mm-role {
    font-size: 9.5px;
    font-weight: 800;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(251, 191, 36, 0.18);
    color: #fbbf24;
  }

  .mm-sub {
    font-size: 10.5px;
    color: var(--text-secondary);
    font-weight: 400;
  }

  .ob-body h3 {
    font-size: 19px;
    font-weight: 800;
    margin-bottom: 6px;
  }

  .ob-body p {
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-secondary);
  }

  .ob-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-top: 4px;
  }

  .ob-dots {
    display: flex;
    gap: 6px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    border: none;
    background: rgba(255, 255, 255, 0.18);
    padding: 0;
    cursor: pointer;
    transition: width 0.2s ease, background 0.2s ease;
  }

  .dot.active {
    width: 22px;
    background: var(--accent);
  }

  .ob-btns {
    display: flex;
    gap: 8px;
  }

  .ob-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 15px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
  }

  .ob-btn.ghost {
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-primary);
  }

  .ob-btn.primary {
    border: none;
    background: var(--accent);
    color: #0b0b0e;
  }
</style>
