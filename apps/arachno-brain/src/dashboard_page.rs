// Shared built-in dashboard page served directly by `arachno-brain` when
// the `--dashboard` command-line option is enabled.
pub const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Arachno Dashboard</title>
  <style>
    :root {
      --bg: #0c1117;
      --panel: rgba(22, 29, 38, 0.86);
      --panel-strong: rgba(18, 24, 32, 0.96);
      --line: rgba(255, 255, 255, 0.1);
      --text: #eef3f7;
      --muted: #94a4b6;
      --accent: #ff9254;
      --accent-soft: rgba(255, 146, 84, 0.18);
      --ok: #65d6a4;
      --warn: #ffc26b;
      --bad: #ff6f61;
      --shadow: 0 18px 50px rgba(0, 0, 0, 0.34);
      --radius: 20px;
    }

    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(255, 146, 84, 0.18), transparent 28rem),
        radial-gradient(circle at bottom right, rgba(70, 138, 255, 0.16), transparent 24rem),
        linear-gradient(160deg, #090c11 0%, #121a22 46%, #0c1117 100%);
      min-height: 100vh;
    }

    body::before {
      content: "";
      position: fixed;
      inset: 0;
      background-image:
        linear-gradient(rgba(255,255,255,0.03) 1px, transparent 1px),
        linear-gradient(90deg, rgba(255,255,255,0.03) 1px, transparent 1px);
      background-size: 28px 28px;
      mask-image: linear-gradient(to bottom, rgba(0,0,0,0.7), transparent);
      pointer-events: none;
    }

    .page {
      max-width: 1780px;
      margin: 0 auto;
      padding: 24px;
    }

    .hero {
      display: flex;
      justify-content: space-between;
      gap: 16px;
      align-items: end;
      margin-bottom: 18px;
    }

    .hero h1 {
      margin: 0;
      font-size: clamp(2rem, 3.6vw, 3.6rem);
      letter-spacing: -0.04em;
    }

    .subtitle {
      color: var(--muted);
      margin-top: 8px;
      max-width: 52rem;
    }

    .badge {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      padding: 10px 14px;
      border-radius: 999px;
      background: rgba(0, 0, 0, 0.24);
      border: 1px solid var(--line);
      box-shadow: var(--shadow);
      color: var(--muted);
      font-size: 0.95rem;
    }

    .badge::before {
      content: "";
      width: 10px;
      height: 10px;
      border-radius: 999px;
      background: var(--warn);
      box-shadow: 0 0 0 0 rgba(255, 194, 107, 0.42);
      animation: pulse 1.6s infinite;
    }

    .badge.ok::before { background: var(--ok); box-shadow: 0 0 0 0 rgba(101, 214, 164, 0.42); }
    .badge.bad::before { background: var(--bad); box-shadow: 0 0 0 0 rgba(255, 111, 97, 0.42); }

    .layout {
      display: grid;
      grid-template-columns: minmax(20rem, 1.2fr) minmax(20rem, 0.8fr);
      gap: 18px;
    }

    .panel {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      box-shadow: var(--shadow);
      backdrop-filter: blur(18px);
      overflow: hidden;
    }

    .panel-header {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: center;
      padding: 18px 20px 0;
    }

    .panel-header h2 {
      margin: 0;
      font-size: 1.15rem;
      letter-spacing: 0.02em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .panel-body {
      padding: 18px 20px 20px;
    }

    .stream-shell {
      background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(0,0,0,0.22));
      border-radius: 18px;
      border: 1px solid rgba(255,255,255,0.08);
      overflow: hidden;
      min-height: 18rem;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .stream-shell img {
      width: 100%;
      height: auto;
      display: block;
      background: #040608;
    }

    .stream-placeholder {
      color: var(--muted);
      padding: 22px;
      text-align: center;
      line-height: 1.5;
    }

    .stats {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
    }

    .stat {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .stat-label {
      color: var(--muted);
      font-size: 0.82rem;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      margin-bottom: 8px;
    }

    .stat-value {
      font-size: 1.35rem;
      font-weight: 700;
      line-height: 1.1;
      word-break: break-word;
    }

    .stat-note {
      color: var(--muted);
      font-size: 0.92rem;
      margin-top: 8px;
      line-height: 1.4;
    }

    .motion-cmd-grid {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 10px;
    }

    @media (max-width: 860px) {
      .motion-cmd-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }

    .motion-btn {
      padding: 14px 12px;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--text);
      border-radius: 12px;
      cursor: pointer;
      font: inherit;
      font-size: 0.97rem;
      transition: transform 120ms ease, border-color 120ms ease, background 120ms ease;
    }

    .motion-btn:hover {
      transform: translateY(-1px);
      border-color: rgba(255, 146, 84, 0.45);
    }

    .motion-btn.active {
      border-color: var(--accent);
      background: var(--accent-soft);
      color: var(--accent);
      font-weight: 600;
    }

    .motion-btn:disabled {
      cursor: not-allowed;
      opacity: 0.45;
      transform: none;
    }

    .manual-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
      margin-bottom: 16px;
    }

    .manual-card {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .manual-card select,
    .manual-card input,
    .manual-card button,
    .manual-sliders button {
      width: 100%;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--text);
      border-radius: 12px;
      padding: 11px 12px;
      font: inherit;
    }

    .manual-card button,
    .manual-sliders button {
      cursor: pointer;
      transition: transform 120ms ease, border-color 120ms ease, background 120ms ease;
    }

    .manual-card button:hover,
    .manual-sliders button:hover {
      transform: translateY(-1px);
      border-color: rgba(255, 146, 84, 0.45);
    }

    .manual-card input[type="number"] {
      appearance: textfield;
    }

    .manual-card button:disabled,
    .manual-sliders button:disabled {
      cursor: not-allowed;
      opacity: 0.55;
      transform: none;
    }

    .manual-sliders {
      display: grid;
      gap: 12px;
    }

    .slider-field {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .slider-top {
      display: flex;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 10px;
    }

    .slider-top strong {
      font-size: 1rem;
      min-width: 4.8rem;
    }

    .slider-value-box {
      display: flex;
      align-items: center;
      gap: 8px;
      color: var(--accent);
      font-weight: 700;
      min-width: 8.25rem;
    }

    .slider-value-box input {
      width: 5.75rem;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--accent);
      border-radius: 10px;
      padding: 6px 8px;
      font: inherit;
      font-weight: 700;
      text-align: right;
    }

    .slider-value-box span {
      color: var(--accent);
      font-weight: 700;
    }

    .slider-field input[type="range"] {
      width: 100%;
      accent-color: var(--accent);
      margin: 0;
    }

    .slider-main-row {
      display: grid;
      grid-template-columns: auto minmax(0, 1fr) auto;
      gap: 12px;
      align-items: center;
    }

    .slider-track {
      min-width: 0;
    }

    .slider-legend {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      color: var(--muted);
      font-size: 0.9rem;
      margin-top: 8px;
    }

    .slider-jump {
      display: grid;
      grid-template-columns: 5.75rem auto;
      gap: 10px;
      align-items: center;
      min-width: 0;
    }

    .slider-jump input {
      width: 100%;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--text);
      border-radius: 10px;
      padding: 8px 10px;
      font: inherit;
    }

    .slider-jump button {
      min-width: 6.2rem;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--text);
      border-radius: 10px;
      padding: 8px 12px;
      font: inherit;
      cursor: pointer;
      transition: transform 120ms ease, border-color 120ms ease, background 120ms ease;
    }

    .slider-jump button:hover {
      transform: translateY(-1px);
      border-color: rgba(255, 146, 84, 0.45);
    }

    .slider-jump button:disabled {
      cursor: not-allowed;
      opacity: 0.55;
      transform: none;
    }

    .manual-actions {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
      margin-top: 4px;
    }

    .manual-actions .wide {
      grid-column: 1 / -1;
    }

    .calibration-actions-layout {
      display: grid;
      grid-template-columns: minmax(0, 1.2fr) minmax(16rem, 0.8fr);
      gap: 12px;
      align-items: stretch;
      margin-top: 4px;
    }

    .calibration-actions-layout .manual-actions {
      margin-top: 0;
      align-content: start;
    }

    .calibration-reference-card {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
      display: flex;
      flex-direction: column;
      gap: 10px;
    }

    .calibration-reference-sketch {
      width: 100%;
      height: 12rem;
      display: block;
      border-radius: 14px;
      background:
        radial-gradient(circle at center, rgba(255, 146, 84, 0.08), transparent 68%),
        rgba(7, 11, 16, 0.7);
      border: 1px solid rgba(255,255,255,0.06);
    }

    .calibration-reference-caption {
      color: var(--muted);
      font-size: 0.86rem;
      line-height: 1.45;
      margin-top: -2px;
    }

    .manual-live-toggle {
      display: flex;
      align-items: center;
      gap: 10px;
      color: var(--muted);
      font-size: 0.95rem;
      margin: 4px 2px 14px;
    }

    .manual-live-toggle input {
      width: 18px;
      height: 18px;
      accent-color: var(--accent);
    }

    .servo-layout {
      margin-top: 18px;
      display: grid;
      gap: 12px;
    }

    .servo-map-shell {
      overflow-x: auto;
      padding-bottom: 6px;
    }

    .servo-map {
      position: relative;
      min-width: 1360px;
      min-height: 760px;
      padding: 28px;
      border-radius: 22px;
      background:
        radial-gradient(circle at 50% 50%, rgba(255, 146, 84, 0.08), transparent 20rem),
        linear-gradient(180deg, rgba(255,255,255,0.03), rgba(0,0,0,0.22));
      border: 1px solid rgba(255,255,255,0.06);
    }

    .servo-orientation {
      color: var(--muted);
      font-size: 0.95rem;
      line-height: 1.5;
    }

    .axis-label {
      position: absolute;
      color: rgba(255,255,255,0.4);
      font-size: 0.78rem;
      letter-spacing: 0.18em;
      text-transform: uppercase;
    }

    .axis-front { top: 16px; left: 50%; transform: translateX(-50%); }
    .axis-rear { bottom: 16px; left: 50%; transform: translateX(-50%); }
    .axis-left {
      top: 50%;
      left: 16px;
      transform: translateY(-50%) rotate(-90deg);
      transform-origin: left top;
    }
    .axis-right {
      top: 50%;
      right: 16px;
      transform: translateY(-50%) rotate(90deg);
      transform-origin: right top;
    }

    .robot-body {
      position: absolute;
      inset: 50% auto auto 50%;
      transform: translate(-50%, -50%);
      width: 14.5rem;
      height: 25rem;
      display: grid;
      place-items: center;
      clip-path: polygon(18% 0%, 82% 0%, 100% 24%, 100% 76%, 82% 100%, 18% 100%, 0% 76%, 0% 24%);
      background:
        linear-gradient(160deg, rgba(255, 146, 84, 0.32), rgba(255, 146, 84, 0.08)),
        linear-gradient(180deg, rgba(255,255,255,0.08), rgba(0,0,0,0.24));
      border: 1px solid rgba(255,255,255,0.14);
      box-shadow:
        inset 0 1px 0 rgba(255,255,255,0.12),
        0 24px 44px rgba(0,0,0,0.28);
    }

    .robot-body::before {
      content: "";
      position: absolute;
      inset: 1.1rem;
      clip-path: inherit;
      background:
        radial-gradient(circle at top, rgba(255,255,255,0.08), transparent 58%),
        linear-gradient(180deg, rgba(0,0,0,0.1), rgba(0,0,0,0.3));
      border: 1px solid rgba(255,255,255,0.08);
    }

    .robot-body-core {
      position: relative;
      z-index: 1;
      display: flex;
      flex-direction: column;
      gap: 8px;
      align-items: center;
      text-align: center;
      padding: 0 1rem;
    }

    .robot-body-title {
      font-size: 1.5rem;
      font-weight: 700;
      letter-spacing: 0.02em;
    }

    .robot-body-note {
      color: rgba(255,255,255,0.68);
      font-size: 0.94rem;
      line-height: 1.45;
    }

    .leg-cluster {
      position: absolute;
      width: 40rem;
    }

    .leg-cluster::after {
      content: "";
      position: absolute;
      top: 50%;
      height: 2px;
      background: linear-gradient(90deg, rgba(255,255,255,0.08), rgba(255, 146, 84, 0.3));
    }

    .leg-cluster.left::after {
      right: -1.9rem;
      width: 1.9rem;
    }

    .leg-cluster.right::after {
      left: -1.9rem;
      width: 1.9rem;
      background: linear-gradient(90deg, rgba(255, 146, 84, 0.3), rgba(255,255,255,0.08));
    }

    .leg-cluster.front-left { top: 6%; left: calc(50% - 50rem); }
    .leg-cluster.middle-left { top: 50%; left: calc(50% - 50rem); transform: translateY(-50%); }
    .leg-cluster.rear-left { bottom: 6%; left: calc(50% - 50rem); }
    .leg-cluster.front-right { top: 6%; right: calc(50% - 50rem); }
    .leg-cluster.middle-right { top: 50%; right: calc(50% - 50rem); transform: translateY(-50%); }
    .leg-cluster.rear-right { bottom: 6%; right: calc(50% - 50rem); }

    .leg-name {
      margin-bottom: 10px;
      font-size: 0.78rem;
      letter-spacing: 0.16em;
      text-transform: uppercase;
      color: rgba(255,255,255,0.54);
    }

    .leg-preview-row {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 8px;
      flex: 1.05 1 0;
      min-width: 0;
    }

    .leg-preview-shell {
      padding: 10px 10px 8px;
      background: rgba(10, 14, 20, 0.62);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .leg-preview-shell.center {
      border-color: rgba(255, 146, 84, 0.18);
      background:
        radial-gradient(circle at center, rgba(255, 146, 84, 0.08), transparent 70%),
        rgba(10, 14, 20, 0.62);
    }

    .leg-preview-top {
      display: flex;
      justify-content: space-between;
      gap: 10px;
      align-items: baseline;
      margin-bottom: 8px;
      color: var(--muted);
      font-size: 0.78rem;
    }

    .leg-preview-svg {
      width: 100%;
      height: 5rem;
      display: block;
    }

    .leg-chain {
      display: flex;
      gap: 10px;
      align-items: stretch;
      flex: 1.55 1 0;
      min-width: 0;
    }

    .leg-chain.reverse {
      flex-direction: row-reverse;
    }

    .leg-cluster-row {
      display: flex;
      gap: 10px;
      align-items: stretch;
    }

    .servo-node {
      flex: 1 1 0;
      min-width: 0;
      min-height: 166px;
      padding: 12px 10px;
      border-radius: 16px;
      background: linear-gradient(180deg, rgba(255,255,255,0.05), rgba(0,0,0,0.24));
      border: 1px solid rgba(255,255,255,0.08);
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
    }

    .servo-node.online { border-color: rgba(101, 214, 164, 0.25); }
    .servo-node.fault { border-color: rgba(255, 111, 97, 0.32); }
    .servo-node.offline {
      border-color: rgba(255,255,255,0.08);
      opacity: 0.78;
    }

    .servo-node-top {
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: start;
      margin-bottom: 10px;
    }

    .joint-name {
      font-size: 0.72rem;
      letter-spacing: 0.1em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .servo-mini-state {
      padding: 5px 8px;
      border-radius: 999px;
      font-size: 0.72rem;
      background: rgba(255,255,255,0.06);
      color: rgba(255,255,255,0.7);
      white-space: nowrap;
    }

    .servo-node-id {
      font-size: 1.28rem;
      font-weight: 700;
      line-height: 1;
      margin-bottom: 6px;
    }

    .servo-node-pos {
      font-size: 0.96rem;
      color: rgba(255,255,255,0.84);
    }

    .servo-node-angle-kind {
      margin-top: 3px;
      font-size: 0.68rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: rgba(148, 164, 182, 0.88);
    }

    .servo-mini-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 8px 10px;
      margin-top: 12px;
    }

    .servo-mini-grid strong {
      display: block;
      color: var(--muted);
      font-size: 0.68rem;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 2px;
    }

    .servo-mini-grid span {
      display: block;
      font-size: 0.84rem;
      color: rgba(255,255,255,0.86);
      line-height: 1.25;
    }

    .servo-node-error {
      margin-top: 12px;
      display: inline-flex;
      max-width: 100%;
      padding: 5px 8px;
      border-radius: 999px;
      font-size: 0.78rem;
      line-height: 1.25;
      background: rgba(255, 111, 97, 0.14);
      color: #ffb7ae;
      border: 1px solid rgba(255, 111, 97, 0.24);
    }

    .muted {
      color: var(--muted);
    }

    @media (max-width: 980px) {
      .layout { grid-template-columns: 1fr; }
      .stats { grid-template-columns: 1fr; }
      .page { padding: 18px; }
    }

    @keyframes pulse {
      0% { box-shadow: 0 0 0 0 currentColor; }
      70% { box-shadow: 0 0 0 10px transparent; }
      100% { box-shadow: 0 0 0 0 transparent; }
    }
  </style>
</head>
<body>
  <div class="page">
    <section class="hero">
      <div>
        <h1>Arachno Debug Dashboard</h1>
        <div class="subtitle">Live visibility into the tethered robot setup: servo feedback, body motion from the IMU bridge, fault states, and the current camera feed.</div>
      </div>
      <div id="status-badge" class="badge">waiting for telemetry</div>
    </section>

    <section class="layout">
      <div class="panel">
        <div class="panel-header">
          <h2>Camera</h2>
          <div class="muted" id="camera-meta">starting...</div>
        </div>
        <div class="panel-body">
          <div id="stream-shell" class="stream-shell">
            <div class="stream-placeholder" id="stream-placeholder">Preparing camera stream...</div>
            <img id="camera-stream" alt="Camera stream" hidden />
          </div>
        </div>
      </div>

      <div class="panel">
        <div class="panel-header">
          <h2>System State</h2>
          <div class="muted" id="updated-at">never</div>
        </div>
        <div class="panel-body">
          <div class="stats">
            <div class="stat">
              <div class="stat-label">Deployment</div>
              <div class="stat-value" id="deployment-profile">-</div>
              <div class="stat-note" id="compute-target">-</div>
            </div>
            <div class="stat">
              <div class="stat-label">Servo Replies</div>
              <div class="stat-value" id="servo-count">0 / 0</div>
              <div class="stat-note">Configured servos currently responding to feedback polls.</div>
            </div>
            <div class="stat">
              <div class="stat-label">Serial Bridge</div>
              <div class="stat-value" id="serial-port">-</div>
              <div class="stat-note" id="serial-note">Waiting for bus state.</div>
            </div>
            <div class="stat">
              <div class="stat-label">Camera Backend</div>
              <div class="stat-value" id="camera-backend">-</div>
              <div class="stat-note" id="camera-note">-</div>
            </div>
            <div class="stat">
              <div class="stat-label">Control Mode</div>
              <div class="stat-value" id="motion-mode">-</div>
              <div class="stat-note" id="motion-summary">-</div>
            </div>
            <div class="stat">
              <div class="stat-label">Safety</div>
              <div class="stat-value" id="safety-status">-</div>
              <div class="stat-note" id="motion-fault">-</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Motion Commands</h2>
        <div class="muted" id="motion-cmd-summary">ready</div>
      </div>
      <div class="panel-body">
        <div class="motion-cmd-grid">
          <button class="motion-btn" id="btn-stand_up" type="button" data-cmd="stand_up">Stand Up</button>
          <button class="motion-btn" id="btn-lay_down" type="button" data-cmd="lay_down">Lay Down</button>
          <button class="motion-btn" id="btn-stand" type="button" data-cmd="stand">Stand</button>
          <button class="motion-btn" id="btn-stop" type="button" data-cmd="stop">Stop</button>
          <button class="motion-btn" id="btn-walk_forward" type="button" data-cmd="walk_forward">Walk Forward</button>
          <button class="motion-btn" id="btn-walk_backward" type="button" data-cmd="walk_backward">Walk Backward</button>
          <button class="motion-btn" id="btn-sidewalk_left" type="button" data-cmd="sidewalk_left">Sidewalk Left</button>
          <button class="motion-btn" id="btn-sidewalk_right" type="button" data-cmd="sidewalk_right">Sidewalk Right</button>
          <button class="motion-btn" id="btn-rotate_left" type="button" data-cmd="rotate_left">Rotate Left</button>
          <button class="motion-btn" id="btn-rotate_right" type="button" data-cmd="rotate_right">Rotate Right</button>
        </div>
        <div class="stat-note" id="motion-cmd-note" style="margin-top: 12px;">
          Commands switch the brain mode immediately. The active mode is highlighted. Safety faults clear on any mode switch.
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>IMU</h2>
        <div class="muted" id="imu-summary">waiting for IMU state</div>
      </div>
      <div class="panel-body">
        <div class="stats">
          <div class="stat">
            <div class="stat-label">Bridge</div>
            <div class="stat-value" id="imu-mode">-</div>
            <div class="stat-note" id="imu-device">-</div>
          </div>
          <div class="stat">
            <div class="stat-label">Sensor</div>
            <div class="stat-value" id="imu-sensor-kind">-</div>
            <div class="stat-note" id="imu-sensor-note">-</div>
          </div>
          <div class="stat">
            <div class="stat-label">Attitude</div>
            <div class="stat-value" id="imu-attitude">-</div>
            <div class="stat-note" id="imu-accel-note">-</div>
          </div>
          <div class="stat">
            <div class="stat-label">Motion</div>
            <div class="stat-value" id="imu-motion">-</div>
            <div class="stat-note" id="imu-health-note">-</div>
          </div>
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Servo Layout</h2>
        <div class="muted" id="fault-summary">No servo data yet</div>
      </div>
      <div class="panel-body">
        <div class="servo-layout">
          <div class="servo-map-shell">
            <div class="servo-map">
              <div class="axis-label axis-front">Front</div>
              <div class="axis-label axis-left">Left</div>
              <div class="axis-label axis-right">Right</div>
              <div class="axis-label axis-rear">Rear</div>
              <div class="robot-body">
                <div class="robot-body-core">
                  <div class="robot-body-title">Hexapod Layout</div>
                  <div class="robot-body-note" id="robot-note">Bird's-eye leg previews show coxa heading and projected reach from femur and tibia.</div>
                </div>
              </div>
              <div id="servo-map-legs"></div>
            </div>
          </div>
          <div class="servo-orientation">
            The map follows the robot's physical layout. Left legs are arms 1-3 from front to back, right legs are arms 4-6. Each cluster combines a top-view live leg preview with the detailed coxa, femur, and tibia telemetry cards.
          </div>
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Manual Control</h2>
        <div class="muted" id="manual-summary">manual control disabled</div>
      </div>
      <div class="panel-body">
        <div class="manual-grid">
          <div class="manual-card">
            <div class="stat-label">Leg Group</div>
            <select id="manual-group"></select>
            <div class="stat-note" id="manual-group-note">Choose a leg group, then set semantic joint angles in degrees.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Manual Mode</div>
            <div class="stat-value" id="manual-mode-state">disabled</div>
            <div class="stat-note" id="manual-mode-note">Start arachno-brain with <code>--mode manual</code> to enable dashboard-based servo control.</div>
          </div>
        </div>

        <div class="manual-sliders">
          <label class="slider-field">
            <div class="slider-top">
              <strong id="manual-coxa-label">Coxa</strong>
            </div>
            <div class="slider-main-row">
              <div class="slider-value-box">
                <input id="manual-coxa-input" type="number" min="-180" max="180" step="0.1" value="0.0" />
                <span>°</span>
              </div>
              <div class="slider-track">
                <input id="manual-coxa-slider" type="range" min="-180" max="180" step="0.5" value="0" />
                <div class="slider-legend">
                  <span id="manual-coxa-negative">back</span>
                  <span id="manual-coxa-positive">forward</span>
                </div>
              </div>
              <div class="slider-jump">
                <input id="manual-coxa-jump" type="number" step="0.1" value="5.0" aria-label="Relative coxa angle jump in degrees" />
                <button id="manual-coxa-jump-apply" type="button">Jump</button>
              </div>
            </div>
          </label>

          <label class="slider-field">
            <div class="slider-top">
              <strong id="manual-femur-label">Femur</strong>
            </div>
            <div class="slider-main-row">
              <div class="slider-value-box">
                <input id="manual-femur-input" type="number" min="-180" max="180" step="0.1" value="0.0" />
                <span>°</span>
              </div>
              <div class="slider-track">
                <input id="manual-femur-slider" type="range" min="-180" max="180" step="0.5" value="0" />
                <div class="slider-legend">
                  <span id="manual-femur-negative">down</span>
                  <span id="manual-femur-positive">up</span>
                </div>
              </div>
              <div class="slider-jump">
                <input id="manual-femur-jump" type="number" step="0.1" value="5.0" aria-label="Relative femur angle jump in degrees" />
                <button id="manual-femur-jump-apply" type="button">Jump</button>
              </div>
            </div>
          </label>

          <label class="slider-field">
            <div class="slider-top">
              <strong id="manual-tibia-label">Tibia</strong>
            </div>
            <div class="slider-main-row">
              <div class="slider-value-box">
                <input id="manual-tibia-input" type="number" min="-180" max="180" step="0.1" value="0.0" />
                <span>°</span>
              </div>
              <div class="slider-track">
                <input id="manual-tibia-slider" type="range" min="-180" max="180" step="0.5" value="0" />
                <div class="slider-legend">
                  <span id="manual-tibia-negative">down</span>
                  <span id="manual-tibia-positive">up</span>
                </div>
              </div>
              <div class="slider-jump">
                <input id="manual-tibia-jump" type="number" step="0.1" value="5.0" aria-label="Relative tibia angle jump in degrees" />
                <button id="manual-tibia-jump-apply" type="button">Jump</button>
              </div>
            </div>
          </label>
        </div>

        <label class="manual-live-toggle">
          <input id="manual-live-apply" type="checkbox" checked />
          <span>Apply slider movement immediately while dragging</span>
        </label>

        <div class="manual-actions">
          <button id="manual-apply" type="button">Apply To Selected Group</button>
          <button id="manual-reset-group" type="button">Reset Selected Group</button>
          <button id="manual-reset-all" class="wide" type="button">Reset All Legs To Manual Zero</button>
          <button id="manual-capture" class="wide" type="button">Capture Current Pose As Manual Zero</button>
          <button id="copy-current-pose" class="wide" type="button">Copy Current Pose To Clipboard</button>
        </div>

        <div class="manual-grid" style="margin-top: 18px;">
          <div class="manual-card">
            <div class="stat-label">Selected Group Torque Limit</div>
            <select id="manual-torque-target">
              <option value="all">All joints</option>
              <option value="coxa">Coxa only</option>
              <option value="femur">Femur only</option>
              <option value="tibia">Tibia only</option>
            </select>
            <div class="stat-note">Choose whether the limit applies to all joints in the selected group or only one joint type.</div>
            <input id="manual-torque-limit" type="number" min="0" max="1000" step="1" value="1000" />
            <div class="stat-note">Applied to the currently selected group after first syncing each target to the live pose.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Manual Utility</div>
            <div class="manual-actions" style="margin-top: 0;">
              <button id="manual-set-torque-limit" class="wide" type="button">Apply Torque Limit To Selected Group</button>
              <button id="manual-sync-current" class="wide" type="button">Set Selected Group Target To Current Pose</button>
            </div>
            <div class="stat-note">Useful before changing resistance or after physically nudging a leg into a new position.</div>
          </div>
        </div>

        <div class="manual-grid" style="margin-top: 18px;">
          <div class="manual-card">
            <div class="stat-label">Semantic Calibration Leg</div>
            <select id="calibration-leg"></select>
            <div class="stat-note" id="calibration-leg-note">Choose a single leg for joint-angle reference capture.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Semantic Calibration Joint</div>
            <select id="calibration-joint"></select>
            <div class="stat-note" id="calibration-joint-note">Captured points adjust the zero reference while keeping the 4096/360 slope fixed.</div>
          </div>
        </div>

        <div class="calibration-actions-layout">
          <div class="manual-actions">
            <button id="calibration-capture-negative" type="button">Capture Negative</button>
            <button id="calibration-capture-zero" type="button">Capture Zero</button>
            <button id="calibration-capture-positive" type="button">Capture Positive</button>
            <button id="calibration-clear-joint" class="wide" type="button">Clear Selected Joint Calibration</button>
            <button id="calibration-reload" class="wide" type="button">Reload Calibration File</button>
          </div>
          <div class="calibration-reference-card">
            <div class="stat-label">Reference Sketch</div>
            <svg id="calibration-reference-sketch" class="calibration-reference-sketch" viewBox="0 0 280 180" aria-label="Calibration reference sketch"></svg>
            <div id="calibration-reference-caption" class="calibration-reference-caption">
              The sketch shows the expected negative, zero, and positive reference poses for the selected joint.
            </div>
          </div>
        </div>
        <div class="stat-note" id="calibration-summary" style="margin-top: 12px;">semantic calibration unavailable</div>
        <div class="stat-note" id="calibration-entry-note" style="margin-top: 8px;">No joint selected yet.</div>
      </div>
    </section>
  </div>

  <script>
    const stateUrl = "/api/state";
    const cameraUrl = "/camera.mjpg";
    const motionCommandUrl = "/api/motion/command";
    const manualApplyUrl = "/api/manual/apply";
    const manualResetUrl = "/api/manual/reset";
    const manualCaptureUrl = "/api/manual/capture";
    const manualTorqueLimitUrl = "/api/manual/torque-limit";
    const manualSyncCurrentUrl = "/api/manual/sync-current";
    const manualJumpUrl = "/api/manual/jump";
    const calibrationCaptureUrl = "/api/calibration/capture";
    const calibrationClearUrl = "/api/calibration/clear";
    const calibrationReloadUrl = "/api/calibration/reload";
    const manualLiveApplyIntervalMs = 200;
    let streamStarted = false;
    let manualGroupsReady = false;
    let manualLiveApplyTimer = null;
    let manualLiveApplyPending = false;
    let lastManualLiveApplyAt = 0;
    let manualSlidersInitialized = { value: false };
    const LEG_ORDER = [
      "front_left",
      "middle_left",
      "rear_left",
      "front_right",
      "middle_right",
      "rear_right",
    ];
    const LEG_META = {
      front_left: { label: "Front left", placement: "front-left left", side: "left" },
      middle_left: { label: "Middle left", placement: "middle-left left", side: "left" },
      rear_left: { label: "Rear left", placement: "rear-left left", side: "left" },
      front_right: { label: "Front right", placement: "front-right right", side: "right" },
      middle_right: { label: "Middle right", placement: "middle-right right", side: "right" },
      rear_right: { label: "Rear right", placement: "rear-right right", side: "right" },
    };
    const ARM_TO_LEG = {
      1: "front_left",
      2: "middle_left",
      3: "rear_left",
      4: "front_right",
      5: "middle_right",
      6: "rear_right",
    };
    const JOINT_LABEL = {
      1: "coxa",
      2: "femur",
      3: "tibia",
    };
    const MANUAL_JOINT_KEYS = ["coxa", "femur", "tibia"];

    function fmt(value, digits = 1) {
      return Number.isFinite(value) ? value.toFixed(digits) : "n/a";
    }

    function legKeyForServo(servo) {
      return ARM_TO_LEG[Math.floor(servo.servo_id / 10)] ?? null;
    }

    function jointIndexForServo(servo) {
      return servo.servo_id % 10;
    }

    function compactError(message) {
      if (!message) return "";
      const lower = message.toLowerCase();
      if (lower.includes("timed out")) return "timeout";
      if (lower.includes("resource busy")) return "busy";
      if (lower.includes("failed to open")) return "bus open failed";
      return message.replace(/^communication failure:\s*/i, "");
    }

    function hexByte(value) {
      return Number.isInteger(value) ? `0x${value.toString(16).padStart(2, "0")}` : "n/a";
    }

    function sliderValue(axis) {
      return Number(document.getElementById(`manual-${axis}-slider`).value);
    }

    function sliderRange(axis) {
      const slider = document.getElementById(`manual-${axis}-slider`);
      return {
        min: Number(slider.min),
        max: Number(slider.max),
      };
    }

    function clampManualAxisValue(axis, value) {
      const { min, max } = sliderRange(axis);
      return Math.min(max, Math.max(min, value));
    }

    function updateSliderReadout(axis) {
      document.getElementById(`manual-${axis}-input`).value = sliderValue(axis).toFixed(1);
    }

    function manualTorqueLimitValue() {
      const value = Number(document.getElementById("manual-torque-limit").value);
      if (!Number.isFinite(value)) return 1000;
      return Math.min(1000, Math.max(0, Math.round(value)));
    }

    function manualTorqueTargetValue() {
      return document.getElementById("manual-torque-target").value || "all";
    }

    function manualJumpValue(axis) {
      const value = Number(document.getElementById(`manual-${axis}-jump`).value);
      return Number.isFinite(value) ? value : 0;
    }

    function manualLiveApplyEnabled() {
      return document.getElementById("manual-live-apply").checked;
    }

    function scheduleLiveManualApply() {
      if (!manualLiveApplyEnabled()) return;
      manualLiveApplyPending = true;
      if (manualLiveApplyTimer) return;

      const now = Date.now();
      const delay = Math.max(0, manualLiveApplyIntervalMs - (now - lastManualLiveApplyAt));
      manualLiveApplyTimer = setTimeout(async () => {
        manualLiveApplyTimer = null;
        if (!manualLiveApplyPending) return;
        manualLiveApplyPending = false;
        lastManualLiveApplyAt = Date.now();
        try {
          await applyManualGroup();
        } catch (error) {
          document.getElementById("manual-summary").textContent = String(error);
        }
        if (manualLiveApplyPending) {
          scheduleLiveManualApply();
        }
      }, delay);
    }

    function syncManualSliderSpecs(joints) {
      for (const joint of joints ?? []) {
        const slider = document.getElementById(`manual-${joint.key}-slider`);
        const input = document.getElementById(`manual-${joint.key}-input`);
        if (!slider) continue;
        slider.min = String(joint.min_deg);
        slider.max = String(joint.max_deg);
        if (input) {
          input.min = String(joint.min_deg);
          input.max = String(joint.max_deg);
        }
        document.getElementById(`manual-${joint.key}-label`).textContent = joint.label;
        document.getElementById(`manual-${joint.key}-negative`).textContent = joint.negative_label;
        document.getElementById(`manual-${joint.key}-positive`).textContent = joint.positive_label;
        updateSliderReadout(joint.key);
      }
    }

    function ensureManualGroups(groups) {
      const select = document.getElementById("manual-group");
      const previous = select.value;
      select.innerHTML = "";
      for (const group of groups ?? []) {
        const option = document.createElement("option");
        option.value = group.key;
        option.textContent = group.label;
        select.appendChild(option);
      }
      if (previous && [...select.options].some((option) => option.value === previous)) {
        select.value = previous;
      }
      manualGroupsReady = select.options.length > 0;
      updateManualGroupNote(groups);
    }

    function updateManualGroupNote(groups) {
      const select = document.getElementById("manual-group");
      const note = document.getElementById("manual-group-note");
      const selected = (groups ?? []).find((group) => group.key === select.value) ?? groups?.[0];
      note.textContent = selected
        ? `${selected.label}: ${selected.legs.join(", ")}`
        : "Choose a leg group, then set semantic joint angles in degrees.";
    }

    function currentManualGroupValue() {
      const groupKey = document.getElementById("manual-group").value;
      return (window.__manualGroupValues ?? []).find((group) => group.key === groupKey) ?? null;
    }

    function currentCalibrationEntry() {
      const legKey = document.getElementById("calibration-leg").value;
      const jointKey = document.getElementById("calibration-joint").value;
      if (legKey === "all") return null;
      return (window.__calibrationEntries ?? []).find((entry) => entry.leg_key === legKey && entry.joint_key === jointKey) ?? null;
    }

    function currentCalibrationJoint() {
      const jointKey = document.getElementById("calibration-joint").value;
      return (window.__calibrationJoints ?? []).find((joint) => joint.key === jointKey) ?? null;
    }

    function currentCalibrationLeg() {
      return document.getElementById("calibration-leg").value || null;
    }

    function syncCalibrationLegs(legs) {
      const select = document.getElementById("calibration-leg");
      const previous = select.value;
      select.innerHTML = "";
      for (const leg of legs ?? []) {
        const option = document.createElement("option");
        option.value = leg.key;
        option.textContent = leg.label;
        select.appendChild(option);
      }
      if (previous && [...select.options].some((option) => option.value === previous)) {
        select.value = previous;
      }
    }

    function syncCalibrationJoints(joints) {
      const select = document.getElementById("calibration-joint");
      const previous = select.value;
      select.innerHTML = "";
      for (const joint of joints ?? []) {
        const option = document.createElement("option");
        option.value = joint.key;
        option.textContent = joint.label;
        select.appendChild(option);
      }
      if (previous && [...select.options].some((option) => option.value === previous)) {
        select.value = previous;
      }
    }

    function updateCalibrationLabels() {
      const joint = currentCalibrationJoint();
      const legKey = currentCalibrationLeg();
      document.getElementById("calibration-leg-note").textContent = joint
        ? legKey === "all"
          ? `Capture ${joint.label.toLowerCase()} reference poses across all legs at once.`
          : `Capture ${joint.label.toLowerCase()} reference poses one leg at a time.`
        : "Choose a leg target for joint-angle reference capture.";
      document.getElementById("calibration-joint-note").textContent = joint
        ? `${joint.negative_label} ${joint.negative_deg.toFixed(0)}°, ${joint.zero_label} ${joint.zero_deg.toFixed(0)}°, ${joint.positive_label} ${joint.positive_deg.toFixed(0)}°. The slope stays fixed at 4096/360.`
        : "Captured points adjust the zero reference while keeping the 4096/360 slope fixed.";
      document.getElementById("calibration-capture-negative").textContent = joint
        ? `Capture ${joint.negative_label} (${joint.negative_deg.toFixed(0)}°)`
        : "Capture Negative";
      document.getElementById("calibration-capture-zero").textContent = joint
        ? `Capture ${joint.zero_label} (${joint.zero_deg.toFixed(0)}°)`
        : "Capture Zero";
      document.getElementById("calibration-capture-positive").textContent = joint
        ? `Capture ${joint.positive_label} (${joint.positive_deg.toFixed(0)}°)`
        : "Capture Positive";
      renderCalibrationReferenceSketch(joint, currentCalibrationLeg());
    }

    function updateCalibrationEntryNote() {
      if (currentCalibrationLeg() === "all") {
        document.getElementById("calibration-entry-note").textContent =
          "Batch mode: capture or clear the selected joint reference across all legs.";
        return;
      }
      const entry = currentCalibrationEntry();
      if (!entry) {
        document.getElementById("calibration-entry-note").textContent =
          "No saved references for the selected joint yet.";
        return;
      }

      const refs = [
        entry.negative_ticks != null ? `neg ${entry.negative_ticks}` : null,
        entry.zero_ticks != null ? `zero ${entry.zero_ticks}` : null,
        entry.positive_ticks != null ? `pos ${entry.positive_ticks}` : null,
      ].filter(Boolean).join(" | ");
      const zeroTick = Number.isFinite(entry.zero_reference_ticks)
        ? `inferred zero ${entry.zero_reference_ticks.toFixed(1)}`
        : "inferred zero n/a";
      const spread = Number.isFinite(entry.max_reference_error_ticks)
        ? `max spread ${entry.max_reference_error_ticks.toFixed(1)} ticks`
        : "need at least 2 references to validate";
      document.getElementById("calibration-entry-note").textContent =
        `${entry.reference_count} reference(s): ${refs || "none"} | ${zeroTick} | ${spread}`;
    }

    function setCalibrationControlsEnabled(enabled) {
      document.getElementById("calibration-leg").disabled = !enabled;
      document.getElementById("calibration-joint").disabled = !enabled;
      document.getElementById("calibration-capture-negative").disabled = !enabled;
      document.getElementById("calibration-capture-zero").disabled = !enabled;
      document.getElementById("calibration-capture-positive").disabled = !enabled;
      document.getElementById("calibration-clear-joint").disabled = !enabled;
      document.getElementById("calibration-reload").disabled = !enabled;
    }

    function polarPoint(origin, length, degrees) {
      const rad = degrees * Math.PI / 180;
      return {
        x: origin.x + Math.cos(rad) * length,
        y: origin.y + Math.sin(rad) * length,
      };
    }

    function calibrationLegMeta(legKey) {
      return LEG_META[legKey] ?? { label: "Selected leg", side: "left" };
    }

    function calibrationSketchForCoxa(joint, legKey) {
      const meta = calibrationLegMeta(legKey);
      const isLeft = meta.side === "left";
      const origin = isLeft ? { x: 188, y: 92 } : { x: 92, y: 92 };
      const radius = 68;
      const baseDegMap = {
        front_left: 225,
        middle_left: 180,
        rear_left: 135,
        front_right: 315,
        middle_right: 0,
        rear_right: 45,
      };
      const baseDeg = baseDegMap[legKey] ?? (isLeft ? 180 : 0);
      const scale = 0.42;
      const angles = {
        negative: baseDeg + (isLeft ? joint.negative_deg * scale : -joint.negative_deg * scale),
        zero: baseDeg + (isLeft ? joint.zero_deg * scale : -joint.zero_deg * scale),
        positive: baseDeg + (isLeft ? joint.positive_deg * scale : -joint.positive_deg * scale),
      };
      const negativeEnd = polarPoint(origin, radius, angles.negative);
      const zeroEnd = polarPoint(origin, radius, angles.zero);
      const positiveEnd = polarPoint(origin, radius, angles.positive);
      const bodyGuideStart = isLeft ? { x: 234, y: 92 } : { x: 46, y: 92 };
      const bodyGuideEnd = isLeft ? { x: 194, y: 92 } : { x: 86, y: 92 };
      const arcStart = polarPoint(origin, 78, Math.min(angles.negative, angles.positive));
      const arcEnd = polarPoint(origin, 78, Math.max(angles.negative, angles.positive));
      const sweep = Math.abs(angles.positive - angles.negative) > 180 ? 1 : 0;
      return `
        <path d='M ${bodyGuideStart.x} ${bodyGuideStart.y} L ${bodyGuideEnd.x} ${bodyGuideEnd.y}'
          fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='14' stroke-linecap='round' />
        <circle cx="${origin.x}" cy="${origin.y}" r="38" fill="rgba(255,255,255,0.03)" stroke="rgba(255,255,255,0.06)" />
        <path d="M ${arcStart.x.toFixed(1)} ${arcStart.y.toFixed(1)} A 78 78 0 0 ${sweep} ${arcEnd.x.toFixed(1)} ${arcEnd.y.toFixed(1)}"
          fill="none" stroke="rgba(255,255,255,0.08)" stroke-dasharray="4 5" />
        <line x1="${origin.x}" y1="${origin.y}" x2="${negativeEnd.x}" y2="${negativeEnd.y}" stroke="rgba(101, 214, 164, 0.9)" stroke-width="5" stroke-linecap="round" />
        <line x1="${origin.x}" y1="${origin.y}" x2="${zeroEnd.x}" y2="${zeroEnd.y}" stroke="rgba(255, 146, 84, 0.95)" stroke-width="6" stroke-linecap="round" />
        <line x1="${origin.x}" y1="${origin.y}" x2="${positiveEnd.x}" y2="${positiveEnd.y}" stroke="rgba(115, 190, 255, 0.92)" stroke-width="5" stroke-linecap="round" />
        <circle cx="${origin.x}" cy="${origin.y}" r="6" fill="rgba(255,255,255,0.92)" />
        <text x="${negativeEnd.x + (isLeft ? -48 : 10)}" y="${negativeEnd.y + 10}" fill="rgba(101, 214, 164, 0.98)" font-size="12" letter-spacing="0.08em">${joint.negative_label.toUpperCase()}</text>
        <text x="${zeroEnd.x + (isLeft ? -14 : 10)}" y="${zeroEnd.y - 10}" fill="rgba(255, 146, 84, 0.98)" font-size="12" letter-spacing="0.08em">${joint.zero_label.toUpperCase()}</text>
        <text x="${positiveEnd.x + (isLeft ? -48 : 10)}" y="${positiveEnd.y + 10}" fill="rgba(115, 190, 255, 0.98)" font-size="12" letter-spacing="0.08em">${joint.positive_label.toUpperCase()}</text>
      `;
    }

    function calibrationSketchForLiftJoint(joint, legKey) {
      const meta = calibrationLegMeta(legKey);
      const isLeft = meta.side === "left";
      const hip = isLeft ? { x: 206, y: 112 } : { x: 74, y: 112 };
      const coxaEnd = isLeft ? { x: 164, y: 112 } : { x: 116, y: 112 };
      const bodyGuideStart = isLeft ? { x: 242, y: 112 } : { x: 38, y: 112 };
      const bodyGuideEnd = isLeft ? { x: 208, y: 112 } : { x: 72, y: 112 };
      const mapAngle = (degrees) => isLeft ? 180 - degrees : degrees;
      const femurReferenceDeg = -8;
      const femurReferenceEnd = pointFrom(
        coxaEnd,
        52,
        mapAngle(femurReferenceDeg) * Math.PI / 180,
      );

      if (joint.key === "tibia") {
        const tibiaLength = 58;
        const tibiaAngles = {
          negative: 56,
          zero: 12,
          positive: -28,
        };
        const negativeEnd = pointFrom(
          femurReferenceEnd,
          tibiaLength,
          mapAngle(tibiaAngles.negative) * Math.PI / 180,
        );
        const zeroEnd = pointFrom(
          femurReferenceEnd,
          tibiaLength,
          mapAngle(tibiaAngles.zero) * Math.PI / 180,
        );
        const positiveEnd = pointFrom(
          femurReferenceEnd,
          tibiaLength,
          mapAngle(tibiaAngles.positive) * Math.PI / 180,
        );
        return `
          <path d='M ${bodyGuideStart.x} ${bodyGuideStart.y} L ${bodyGuideEnd.x} ${bodyGuideEnd.y}'
            fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='14' stroke-linecap='round' />
          <line x1="22" y1="${hip.y}" x2="258" y2="${hip.y}" stroke="rgba(255,255,255,0.08)" stroke-dasharray="5 5" />
          <circle cx="${hip.x}" cy="${hip.y}" r="7" fill="rgba(255,255,255,0.92)" />
          <line x1="${hip.x}" y1="${hip.y}" x2="${coxaEnd.x}" y2="${coxaEnd.y}" stroke="rgba(255,255,255,0.24)" stroke-width="6" stroke-linecap="round" />
          <line x1="${coxaEnd.x}" y1="${coxaEnd.y}" x2="${femurReferenceEnd.x}" y2="${femurReferenceEnd.y}" stroke="rgba(255, 146, 84, 0.42)" stroke-width="5" stroke-linecap="round" />
          <circle cx="${femurReferenceEnd.x}" cy="${femurReferenceEnd.y}" r="4" fill="rgba(255,255,255,0.78)" />
          <line x1="${femurReferenceEnd.x}" y1="${femurReferenceEnd.y}" x2="${negativeEnd.x}" y2="${negativeEnd.y}" stroke="rgba(101, 214, 164, 0.9)" stroke-width="5" stroke-linecap="round" />
          <line x1="${femurReferenceEnd.x}" y1="${femurReferenceEnd.y}" x2="${zeroEnd.x}" y2="${zeroEnd.y}" stroke="rgba(255, 146, 84, 0.95)" stroke-width="6" stroke-linecap="round" />
          <line x1="${femurReferenceEnd.x}" y1="${femurReferenceEnd.y}" x2="${positiveEnd.x}" y2="${positiveEnd.y}" stroke="rgba(115, 190, 255, 0.92)" stroke-width="5" stroke-linecap="round" />
          <text x="${femurReferenceEnd.x + (isLeft ? -78 : 10)}" y="${femurReferenceEnd.y - 10}" fill="rgba(255, 146, 84, 0.72)" font-size="11" letter-spacing="0.06em">FEMUR ZERO</text>
          <text x="${negativeEnd.x + (isLeft ? -52 : 8)}" y="${negativeEnd.y + 14}" fill="rgba(101, 214, 164, 0.98)" font-size="12" letter-spacing="0.08em">${joint.negative_label.toUpperCase()}</text>
          <text x="${zeroEnd.x + (isLeft ? -52 : 8)}" y="${zeroEnd.y - 8}" fill="rgba(255, 146, 84, 0.98)" font-size="12" letter-spacing="0.08em">${joint.zero_label.toUpperCase()}</text>
          <text x="${positiveEnd.x + (isLeft ? -52 : 8)}" y="${positiveEnd.y - 10}" fill="rgba(115, 190, 255, 0.98)" font-size="12" letter-spacing="0.08em">${joint.positive_label.toUpperCase()}</text>
        `;
      }

      const length = 104;
      const angles = {
        negative: 30,
        zero: -8,
        positive: -46,
      };
      const negativeEnd = pointFrom(coxaEnd, length, mapAngle(angles.negative) * Math.PI / 180);
      const zeroEnd = pointFrom(coxaEnd, length, mapAngle(angles.zero) * Math.PI / 180);
      const positiveEnd = pointFrom(coxaEnd, length, mapAngle(angles.positive) * Math.PI / 180);
      return `
        <path d='M ${bodyGuideStart.x} ${bodyGuideStart.y} L ${bodyGuideEnd.x} ${bodyGuideEnd.y}'
          fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='14' stroke-linecap='round' />
        <line x1="22" y1="${hip.y}" x2="258" y2="${hip.y}" stroke="rgba(255,255,255,0.08)" stroke-dasharray="5 5" />
        <circle cx="${hip.x}" cy="${hip.y}" r="7" fill="rgba(255,255,255,0.92)" />
        <line x1="${hip.x}" y1="${hip.y}" x2="${coxaEnd.x}" y2="${coxaEnd.y}" stroke="rgba(255,255,255,0.24)" stroke-width="6" stroke-linecap="round" />
        <line x1="${coxaEnd.x}" y1="${coxaEnd.y}" x2="${negativeEnd.x}" y2="${negativeEnd.y}" stroke="rgba(101, 214, 164, 0.9)" stroke-width="5" stroke-linecap="round" />
        <line x1="${coxaEnd.x}" y1="${coxaEnd.y}" x2="${zeroEnd.x}" y2="${zeroEnd.y}" stroke="rgba(255, 146, 84, 0.95)" stroke-width="6" stroke-linecap="round" />
        <line x1="${coxaEnd.x}" y1="${coxaEnd.y}" x2="${positiveEnd.x}" y2="${positiveEnd.y}" stroke="rgba(115, 190, 255, 0.92)" stroke-width="5" stroke-linecap="round" />
        <text x="${negativeEnd.x + (isLeft ? -52 : 8)}" y="${negativeEnd.y + 14}" fill="rgba(101, 214, 164, 0.98)" font-size="12" letter-spacing="0.08em">${joint.negative_label.toUpperCase()}</text>
        <text x="${zeroEnd.x + (isLeft ? -52 : 8)}" y="${zeroEnd.y - 8}" fill="rgba(255, 146, 84, 0.98)" font-size="12" letter-spacing="0.08em">${joint.zero_label.toUpperCase()}</text>
        <text x="${positiveEnd.x + (isLeft ? -52 : 8)}" y="${positiveEnd.y - 10}" fill="rgba(115, 190, 255, 0.98)" font-size="12" letter-spacing="0.08em">${joint.positive_label.toUpperCase()}</text>
      `;
    }

    function renderCalibrationReferenceSketch(joint, legKey) {
      const sketch = document.getElementById("calibration-reference-sketch");
      const caption = document.getElementById("calibration-reference-caption");
      if (!sketch || !caption) return;

      if (!joint) {
        sketch.innerHTML = `
          <rect x="18" y="18" width="244" height="144" rx="18" fill="rgba(255,255,255,0.02)" stroke="rgba(255,255,255,0.06)" />
          <text x="140" y="82" text-anchor="middle" fill="rgba(255,255,255,0.82)" font-size="15">Select a joint</text>
          <text x="140" y="106" text-anchor="middle" fill="rgba(148,164,182,0.92)" font-size="12">to see the expected reference poses</text>
        `;
        caption.textContent = "The sketch shows the expected negative, zero, and positive reference poses for the selected joint.";
        return;
      }

      const title = `${joint.label} reference positions`;
      const degrees = `${joint.negative_deg.toFixed(0)}° / ${joint.zero_deg.toFixed(0)}° / ${joint.positive_deg.toFixed(0)}°`;
      const body = joint.key === "coxa"
        ? calibrationSketchForCoxa(joint, legKey)
        : calibrationSketchForLiftJoint(joint, legKey);
      sketch.innerHTML = `
        <text x="18" y="22" fill="rgba(255,255,255,0.88)" font-size="13" letter-spacing="0.08em">${title.toUpperCase()}</text>
        <text x="18" y="40" fill="rgba(148,164,182,0.92)" font-size="11">${degrees}</text>
        ${body}
      `;
      const legLabel = calibrationLegMeta(legKey).label.toLowerCase();
      caption.textContent = joint.key === "coxa"
        ? `Top view for the ${legLabel}: negative and positive swing around the zero heading.`
        : joint.key === "tibia"
          ? `Outer side view for the ${legLabel}: tibia references are shown relative to the femur zero pose.`
          : `Outer side view for the ${legLabel}: negative is lower than zero, positive is higher than zero.`;
    }

    function escapeTomlString(value) {
      return String(value).replaceAll("\\", "\\\\").replaceAll("\"", "\\\"");
    }

    function servoByJoint(servos) {
      return Object.fromEntries(servos.map((servo) => [jointIndexForServo(servo), servo]));
    }

    function currentPoseToml(state) {
      const grouped = groupServosByLeg(state?.servos ?? []);
      const lines = [
        "[pose]",
        `captured_at = "${escapeTomlString(new Date().toISOString())}"`,
        `robot_name = "${escapeTomlString(state?.robot_name ?? "arachno")}"`,
        `motion_mode = "${escapeTomlString(state?.motion_mode ?? "unknown")}"`,
        "",
      ];

      for (const legKey of LEG_ORDER) {
        const byJoint = servoByJoint(grouped[legKey] ?? []);
        const meta = LEG_META[legKey];
        lines.push(`[pose.${legKey}]`);
        lines.push(`label = "${escapeTomlString(meta.label)}"`);
        for (const [jointIndex, jointKey] of [[1, "coxa"], [2, "femur"], [3, "tibia"]]) {
          const servo = byJoint[jointIndex];
          if (!servo?.telemetry) {
            lines.push(`# ${jointKey}_ticks unavailable: servo offline or no live feedback`);
            continue;
          }
          lines.push(`${jointKey}_servo_id = ${servo.servo_id}`);
          lines.push(`${jointKey}_ticks = ${servo.telemetry.present_position_ticks}`);
          if (Number.isFinite(servo.position_deg)) {
            lines.push(`${jointKey}_absolute_deg = ${servo.position_deg.toFixed(2)}`);
          }
          if (Number.isFinite(servo.semantic_angle_deg)) {
            lines.push(`${jointKey}_semantic_deg = ${servo.semantic_angle_deg.toFixed(2)}`);
          }
        }
        lines.push("");
      }

      return lines.join("\n").trimEnd() + "\n";
    }

    async function copyTextToClipboard(text) {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        return;
      }

      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.setAttribute("readonly", "");
      textarea.style.position = "fixed";
      textarea.style.top = "-1000px";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
    }

    function setManualSlidersFromGroupValue(force = false) {
      const group = currentManualGroupValue();
      if (!group) return;
      if (!force && manualSlidersInitialized.value) return;
      document.getElementById("manual-coxa-slider").value = String(group.coxa_deg.toFixed(1));
      document.getElementById("manual-femur-slider").value = String(group.femur_deg.toFixed(1));
      document.getElementById("manual-tibia-slider").value = String(group.tibia_deg.toFixed(1));
      for (const axis of MANUAL_JOINT_KEYS) {
        updateSliderReadout(axis);
      }
      manualSlidersInitialized.value = true;
    }

    function setManualControlsEnabled(enabled) {
      document.getElementById("manual-group").disabled = !enabled;
      document.getElementById("manual-apply").disabled = !enabled;
      document.getElementById("manual-reset-group").disabled = !enabled;
      document.getElementById("manual-reset-all").disabled = !enabled;
      document.getElementById("manual-capture").disabled = !enabled;
      document.getElementById("manual-set-torque-limit").disabled = !enabled;
      document.getElementById("manual-sync-current").disabled = !enabled;
      document.getElementById("manual-torque-target").disabled = !enabled;
      document.getElementById("manual-torque-limit").disabled = !enabled;
      document.getElementById("manual-live-apply").disabled = !enabled;
      for (const axis of MANUAL_JOINT_KEYS) {
        document.getElementById(`manual-${axis}-slider`).disabled = !enabled;
        document.getElementById(`manual-${axis}-input`).disabled = !enabled;
        document.getElementById(`manual-${axis}-jump`).disabled = !enabled;
        document.getElementById(`manual-${axis}-jump-apply`).disabled = !enabled;
      }
    }

    async function postJson(url, payload) {
      const response = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload ?? {}),
      });
      if (!response.ok) {
        const message = await response.text();
        throw new Error(message || `${response.status}`);
      }
      return response.json();
    }

    async function sendMotionCommand(command) {
      const summaryEl = document.getElementById("motion-cmd-summary");
      summaryEl.textContent = "sending…";
      try {
        const result = await postJson(motionCommandUrl, { command });
        summaryEl.textContent = result.summary;
      } catch (err) {
        summaryEl.textContent = `error: ${err.message}`;
      }
      await refresh();
    }

    function updateMotionButtons(motionMode) {
      const modeToCmd = {
        stand_up: "stand_up",
        lay_down: "lay_down",
        stand: "stand",
        slow_walk: "walk_forward",
        backward_walk: "walk_backward",
        rotate_left: "rotate_left",
        rotate_right: "rotate_right",
        sidewalk_left: "sidewalk_left",
        sidewalk_right: "sidewalk_right",
      };
      const activeCmd = modeToCmd[motionMode] ?? null;
      document.querySelectorAll(".motion-btn").forEach(btn => {
        const cmd = btn.dataset.cmd;
        btn.classList.toggle("active", cmd === activeCmd || (cmd === "stop" && motionMode === "stand"));
      });
    }

    document.querySelectorAll(".motion-btn").forEach(btn => {
      btn.addEventListener("click", () => sendMotionCommand(btn.dataset.cmd));
    });

    async function applyManualGroup() {
      const result = await postJson(manualApplyUrl, {
        group_key: document.getElementById("manual-group").value,
        coxa_deg: sliderValue("coxa"),
        femur_deg: sliderValue("femur"),
        tibia_deg: sliderValue("tibia"),
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
    }

    async function applyManualTorqueLimit() {
      const result = await postJson(manualTorqueLimitUrl, {
        group_key: document.getElementById("manual-group").value,
        target: manualTorqueTargetValue(),
        torque_limit: manualTorqueLimitValue(),
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
    }

    async function syncManualTargetToCurrent() {
      const result = await postJson(manualSyncCurrentUrl, {
        group_key: document.getElementById("manual-group").value,
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
    }

    function setManualAxisFromInput(axis) {
      const input = document.getElementById(`manual-${axis}-input`);
      const slider = document.getElementById(`manual-${axis}-slider`);
      const value = Number(input.value);
      if (!Number.isFinite(value)) {
        updateSliderReadout(axis);
        return;
      }
      const clamped = clampManualAxisValue(axis, value);
      slider.value = String(clamped);
      updateSliderReadout(axis);
    }

    async function applyManualAxisJump(axis) {
      const delta = manualJumpValue(axis);
      const result = await postJson(manualJumpUrl, {
        group_key: document.getElementById("manual-group").value,
        joint_key: axis,
        delta_deg: delta,
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
    }

    async function resetManualGroup() {
      const result = await postJson(manualResetUrl, {
        group_key: document.getElementById("manual-group").value,
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
    }

    async function resetManualAll() {
      const result = await postJson(manualResetUrl, {});
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
    }

    async function captureManualZero() {
      const result = await postJson(manualCaptureUrl, {});
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
    }

    async function copyCurrentPose() {
      const state = window.__latestState;
      if (!state?.servos?.length) {
        throw new Error("no live servo state available yet");
      }
      await copyTextToClipboard(currentPoseToml(state));
      document.getElementById("manual-summary").textContent =
        `copied current pose for ${state.online_servo_count}/${state.servos.length} servos to clipboard`;
    }

    async function captureCalibrationReference(referenceKey) {
      const result = await postJson(calibrationCaptureUrl, {
        leg_key: document.getElementById("calibration-leg").value,
        joint_key: document.getElementById("calibration-joint").value,
        reference_key: referenceKey,
      });
      document.getElementById("calibration-summary").textContent = result.summary;
      await refresh();
    }

    async function clearCalibrationJoint() {
      const result = await postJson(calibrationClearUrl, {
        leg_key: document.getElementById("calibration-leg").value,
        joint_key: document.getElementById("calibration-joint").value,
      });
      document.getElementById("calibration-summary").textContent = result.summary;
      await refresh();
    }

    async function reloadCalibrationFile() {
      const result = await postJson(calibrationReloadUrl, {});
      document.getElementById("calibration-summary").textContent = result.summary;
      await refresh();
    }

    function bindManualControls() {
      if (window.__manualControlsBound) return;
      window.__manualControlsBound = true;
      for (const axis of MANUAL_JOINT_KEYS) {
        const slider = document.getElementById(`manual-${axis}-slider`);
        slider.addEventListener("input", () => {
          updateSliderReadout(axis);
          scheduleLiveManualApply();
        });
        slider.addEventListener("change", () => {
          updateSliderReadout(axis);
        });
        const input = document.getElementById(`manual-${axis}-input`);
        input.addEventListener("input", () => {
          setManualAxisFromInput(axis);
          scheduleLiveManualApply();
        });
        input.addEventListener("change", () => {
          setManualAxisFromInput(axis);
        });
        const jumpInput = document.getElementById(`manual-${axis}-jump`);
        const jumpButton = document.getElementById(`manual-${axis}-jump-apply`);
        jumpButton.addEventListener("click", () => {
          applyManualAxisJump(axis).catch((error) => {
            document.getElementById("manual-summary").textContent = String(error);
          });
        });
        jumpInput.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyManualAxisJump(axis).catch((error) => {
              document.getElementById("manual-summary").textContent = String(error);
            });
          }
        });
        updateSliderReadout(axis);
      }
      document.getElementById("manual-group").addEventListener("change", () => {
        updateManualGroupNote(window.__manualGroups ?? []);
        setManualSlidersFromGroupValue(true);
      });
      document.getElementById("manual-apply").addEventListener("click", () => applyManualGroup().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("manual-reset-group").addEventListener("click", () => resetManualGroup().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("manual-reset-all").addEventListener("click", () => resetManualAll().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("manual-capture").addEventListener("click", () => captureManualZero().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("copy-current-pose").addEventListener("click", () => copyCurrentPose().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("manual-set-torque-limit").addEventListener("click", () => applyManualTorqueLimit().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("manual-sync-current").addEventListener("click", () => syncManualTargetToCurrent().catch((error) => {
        document.getElementById("manual-summary").textContent = String(error);
      }));
      document.getElementById("calibration-leg").addEventListener("change", () => {
        updateCalibrationLabels();
        updateCalibrationEntryNote();
      });
      document.getElementById("calibration-joint").addEventListener("change", () => {
        updateCalibrationLabels();
        updateCalibrationEntryNote();
      });
      document.getElementById("calibration-capture-negative").addEventListener("click", () => captureCalibrationReference("negative").catch((error) => {
        document.getElementById("calibration-summary").textContent = String(error);
      }));
      document.getElementById("calibration-capture-zero").addEventListener("click", () => captureCalibrationReference("zero").catch((error) => {
        document.getElementById("calibration-summary").textContent = String(error);
      }));
      document.getElementById("calibration-capture-positive").addEventListener("click", () => captureCalibrationReference("positive").catch((error) => {
        document.getElementById("calibration-summary").textContent = String(error);
      }));
      document.getElementById("calibration-clear-joint").addEventListener("click", () => clearCalibrationJoint().catch((error) => {
        document.getElementById("calibration-summary").textContent = String(error);
      }));
      document.getElementById("calibration-reload").addEventListener("click", () => reloadCalibrationFile().catch((error) => {
        document.getElementById("calibration-summary").textContent = String(error);
      }));
    }

    function updateManualPanel(manual) {
      window.__manualGroups = manual?.groups ?? [];
      window.__manualGroupValues = manual?.group_values ?? [];
      bindManualControls();
      ensureManualGroups(window.__manualGroups);
      syncManualSliderSpecs(manual?.joints ?? []);
      setManualSlidersFromGroupValue(!manualSlidersInitialized.value);

      document.getElementById("manual-summary").textContent = manual?.summary ?? "manual control unavailable";
      document.getElementById("manual-mode-state").textContent = manual?.enabled
        ? (manual.ready ? "ready" : "waiting")
        : "disabled";
      document.getElementById("manual-mode-note").textContent = manual?.enabled
        ? (manual.base_pose_captured
            ? "Manual zero is captured for reset actions. Sliders show absolute semantic angles."
            : "Sliders show absolute semantic angles. Capture the current pose if you want reset-to-zero behavior.")
        : "Start arachno-brain with --mode manual to enable dashboard-based servo control.";

      setManualControlsEnabled(Boolean(manual?.enabled && manualGroupsReady));
    }

    function updateCalibrationPanel(calibration) {
      window.__calibrationEntries = calibration?.entries ?? [];
      window.__calibrationJoints = calibration?.joints ?? [];
      bindManualControls();
      syncCalibrationLegs(calibration?.legs ?? []);
      syncCalibrationJoints(calibration?.joints ?? []);
      updateCalibrationLabels();
      updateCalibrationEntryNote();
      document.getElementById("calibration-summary").textContent = calibration?.summary ?? "semantic calibration unavailable";
      setCalibrationControlsEnabled(Boolean(calibration?.enabled));
    }

    function updateImuPanel(imu) {
      if (!imu) {
        document.getElementById("imu-summary").textContent = "IMU disabled";
        document.getElementById("imu-mode").textContent = "disabled";
        document.getElementById("imu-device").textContent = "No IMU section is configured for this profile.";
        document.getElementById("imu-sensor-kind").textContent = "-";
        document.getElementById("imu-sensor-note").textContent = "-";
        document.getElementById("imu-attitude").textContent = "-";
        document.getElementById("imu-accel-note").textContent = "-";
        document.getElementById("imu-motion").textContent = "-";
        document.getElementById("imu-health-note").textContent = "-";
        return;
      }

      const sensorKind = imu.sensor_kind ?? (imu.enabled ? "probing..." : "disabled");
      const sensorNote = [
        imu.sample_hz ? `${imu.sample_hz} Hz` : null,
        imu.spi_mode != null ? `SPI mode ${imu.spi_mode}` : null,
        imu.observed_who_am_i != null ? `WHO_AM_I ${hexByte(imu.observed_who_am_i)}` : null,
      ].filter(Boolean).join(" | ") || "Waiting for firmware info.";

      const attitude = imu.roll_deg != null && imu.pitch_deg != null
        ? `roll ${fmt(imu.roll_deg, 1)}° / pitch ${fmt(imu.pitch_deg, 1)}°`
        : "waiting for sample";
      const accelNote = imu.accel_norm_mps2 != null
        ? `|a| ${fmt(imu.accel_norm_mps2, 2)} m/s²`
        : "No accelerometer sample yet.";
      const motion = imu.gyro_norm_deg_s != null
        ? `${fmt(imu.gyro_norm_deg_s, 1)} °/s`
        : "waiting for sample";
      const faults = imu.telemetry?.faults?.length ? imu.telemetry.faults.join(", ") : "ok";
      const healthBits = [
        imu.telemetry?.temperature_c != null ? `temp ${fmt(imu.telemetry.temperature_c, 1)} °C` : null,
        `faults ${faults}`,
        imu.last_error ? compactError(imu.last_error) : null,
      ].filter(Boolean).join(" | ");

      document.getElementById("imu-summary").textContent = imu.last_error ?? `${sensorKind} streaming`;
      document.getElementById("imu-mode").textContent = imu.enabled ? imu.mode : "disabled";
      document.getElementById("imu-device").textContent = imu.device ?? imu.description ?? "No device path";
      document.getElementById("imu-sensor-kind").textContent = sensorKind;
      document.getElementById("imu-sensor-note").textContent = sensorNote;
      document.getElementById("imu-attitude").textContent = attitude;
      document.getElementById("imu-accel-note").textContent = accelNote;
      document.getElementById("imu-motion").textContent = motion;
      document.getElementById("imu-health-note").textContent = healthBits || "No telemetry yet.";
    }

    function renderServoNode(servo) {
      const telemetry = servo.telemetry;
      const faults = telemetry?.faults ?? [];
      const classes = ["servo-node"];
      if (!servo.online) {
        classes.push("offline");
      } else if (faults.length) {
        classes.push("fault");
      } else {
        classes.push("online");
      }

      const jointIndex = jointIndexForServo(servo);
      const jointLabel = JOINT_LABEL[jointIndex] ?? "joint";
      const load = telemetry ? `${fmt(telemetry.present_load_pct)}%` : "n/a";
      const voltage = telemetry ? `${fmt(telemetry.present_voltage_v, 1)} V` : "n/a";
      const current = telemetry?.present_current_ma != null ? `${telemetry.present_current_ma} mA` : "n/a";
      const temp = telemetry?.present_temperature_c != null ? `${telemetry.present_temperature_c} °C` : "n/a";
      const stateLabel = telemetry ? (telemetry.moving ? "moving" : "ready") : "offline";
      const errorText = compactError(servo.error);
      const displayAngle = servo.semantic_angle_deg ?? servo.position_deg;
      const displayAngleLabel = servo.semantic_angle_deg != null ? "semantic" : "absolute";

      return `
        <article class="${classes.join(" ")}">
          <div class="servo-node-top">
            <div>
              <div class="joint-name">${jointLabel}</div>
              <div class="servo-node-id">${servo.servo_id}</div>
              <div class="servo-node-pos">${displayAngle != null ? `${fmt(displayAngle, 1)}°` : "n/a"}</div>
              <div class="servo-node-angle-kind">${displayAngleLabel}</div>
            </div>
            <div class="servo-mini-state">${stateLabel}</div>
          </div>
          <div class="servo-mini-grid">
            <div><strong>Load</strong><span>${load}</span></div>
            <div><strong>Volt</strong><span>${voltage}</span></div>
            <div><strong>Temp</strong><span>${temp}</span></div>
            <div><strong>Current</strong><span>${current}</span></div>
          </div>
          ${errorText ? `<div class="servo-node-error">${errorText}</div>` : ""}
        </article>
      `;
    }

    function renderLegCluster(legKey, servos) {
      const meta = LEG_META[legKey];
      const sorted = [...servos].sort((left, right) => jointIndexForServo(left) - jointIndexForServo(right));
      const chainClass = meta.side === "left" ? "leg-chain reverse" : "leg-chain";
      const preview = renderLegPreviewRow(legKey, sorted);
      const chain = `
        <div class="${chainClass}">
          ${sorted.map(renderServoNode).join("")}
        </div>
      `;
      const clusterRow = meta.side === "left"
        ? `${chain}${preview}`
        : `${preview}${chain}`;

      return `
        <section class="leg-cluster ${meta.placement}">
          <div class="leg-name">${meta.label}</div>
          <div class="leg-cluster-row">
            ${clusterRow}
          </div>
        </section>
      `;
    }

    function renderServoMap(servos) {
      const grouped = groupServosByLeg(servos);
      return LEG_ORDER.map((legKey) => renderLegCluster(legKey, grouped[legKey])).join("");
    }

    function groupServosByLeg(servos) {
      const grouped = Object.fromEntries(LEG_ORDER.map((key) => [key, []]));
      for (const servo of servos) {
        const legKey = legKeyForServo(servo);
        if (legKey && grouped[legKey]) {
          grouped[legKey].push(servo);
        }
      }
      return grouped;
    }

    function clamp(value, min, max) {
      return Math.min(Math.max(value, min), max);
    }

    function pointFrom(origin, length, angleRad) {
      return {
        x: origin.x + Math.cos(angleRad) * length,
        y: origin.y + Math.sin(angleRad) * length,
      };
    }

    function currentLegPreview(legKey) {
      return (window.__legPreviews ?? []).find((preview) => preview.leg_key === legKey) ?? null;
    }

    function fitPreviewPose(rawPose, width = 220, height = 116) {
      const points = [rawPose.anchor, rawPose.coxa_end, rawPose.femur_end, rawPose.tibia_end];
      const minX = Math.min(...points.map((point) => point.x));
      const maxX = Math.max(...points.map((point) => point.x));
      const minY = Math.min(...points.map((point) => point.y));
      const maxY = Math.max(...points.map((point) => point.y));
      const spanX = Math.max(maxX - minX, 1);
      const spanY = Math.max(maxY - minY, 1);
      const marginX = 26;
      const marginY = 18;
      const scale = Math.min(
        (width - marginX * 2) / spanX,
        (height - marginY * 2) / spanY,
      );
      const offsetX = (width - spanX * scale) / 2 - minX * scale;
      const offsetY = (height - spanY * scale) / 2 - minY * scale;
      const mapPoint = (point) => ({
        x: offsetX + point.x * scale,
        y: offsetY + point.y * scale,
      });

      return {
        anchor: mapPoint(rawPose.anchor),
        coxaEnd: mapPoint(rawPose.coxa_end),
        femurEnd: mapPoint(rawPose.femur_end),
        tibiaEnd: mapPoint(rawPose.tibia_end),
      };
    }

    function previewPlaceholder(title, count, label) {
      return `
        <div class="leg-preview-shell ${label}">
          <div class="leg-preview-top">
            <strong>${title}</strong>
            <span>${count}/3</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 220 116" aria-label="${title} pose unavailable">
            <rect x="28" y="22" width="164" height="72" rx="16" fill="rgba(255,255,255,0.03)" stroke="rgba(255,255,255,0.08)" />
            <text x="110" y="58" text-anchor="middle" fill="rgba(238,243,247,0.78)" font-size="12">preview unavailable</text>
            <text x="110" y="76" text-anchor="middle" fill="rgba(148,164,182,0.92)" font-size="11">need fresh semantic telemetry</text>
          </svg>
        </div>
      `;
    }

    function renderLegBirdPreview(legKey, servos) {
      const meta = LEG_META[legKey];
      const onlineCount = servos.filter((servo) => servo.online).length;
      const rawPose = currentLegPreview(legKey)?.top_view;
      if (!rawPose) {
        return previewPlaceholder("Top view", onlineCount, "center");
      }
      const pose = fitPreviewPose(rawPose);
      const stroke = onlineCount === 3 ? '#ff9254' : (onlineCount > 0 ? '#ffc26b' : '#5a6775');
      const fill = onlineCount === 3 ? 'rgba(255,146,84,0.12)' : 'rgba(255,255,255,0.04)';
      const inwardDx = Math.sign(pose.anchor.x - pose.coxaEnd.x) || (meta.side === "left" ? 1 : -1);
      const bodyGuideStart = { x: pose.anchor.x + inwardDx * 20, y: pose.anchor.y };
      const bodyGuideEnd = { x: pose.anchor.x + inwardDx * 4, y: pose.anchor.y };

      return `
        <div class="leg-preview-shell center">
          <div class="leg-preview-top">
            <strong>Top view</strong>
            <span>${onlineCount}/3 online</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 220 116" aria-label="${meta.label} top-view live pose">
            <path d='M ${bodyGuideStart.x.toFixed(1)} ${bodyGuideStart.y.toFixed(1)} L ${bodyGuideEnd.x.toFixed(1)} ${bodyGuideEnd.y.toFixed(1)}'
              fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='10' stroke-linecap='round' />
            <circle cx='${pose.anchor.x}' cy='${pose.anchor.y}' r='9' fill='${fill}' stroke='rgba(255,255,255,0.10)' />
            <path d='M ${pose.anchor.x} ${pose.anchor.y} L ${pose.coxaEnd.x.toFixed(1)} ${pose.coxaEnd.y.toFixed(1)} L ${pose.femurEnd.x.toFixed(1)} ${pose.femurEnd.y.toFixed(1)} L ${pose.tibiaEnd.x.toFixed(1)} ${pose.tibiaEnd.y.toFixed(1)}'
              fill='none' stroke='${stroke}' stroke-width='9' stroke-linecap='round' stroke-linejoin='round' />
            <circle cx='${pose.anchor.x}' cy='${pose.anchor.y}' r='6.5' fill='#eef3f7' />
            <circle cx='${pose.coxaEnd.x.toFixed(1)}' cy='${pose.coxaEnd.y.toFixed(1)}' r='5.5' fill='#d9e2ec' />
            <circle cx='${pose.femurEnd.x.toFixed(1)}' cy='${pose.femurEnd.y.toFixed(1)}' r='5.2' fill='#c8d3de' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='5.2' fill='${stroke}' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='9' fill='none' stroke='${stroke}' stroke-width='1.6' opacity='0.5' />
          </svg>
        </div>
      `;
    }

    function renderLegSidePreview(legKey, servos) {
      const meta = LEG_META[legKey];
      const onlineCount = servos.filter((servo) => servo.online).length;
      const rawPose = currentLegPreview(legKey)?.side_view;
      if (!rawPose) {
        return previewPlaceholder("Side view", onlineCount, "outer");
      }
      const pose = fitPreviewPose(rawPose);
      const stroke = onlineCount === 3 ? '#7dc8ff' : (onlineCount > 0 ? '#b8dfff' : '#5a6775');
      const inwardDx = Math.sign(pose.anchor.x - pose.coxaEnd.x) || (meta.side === "left" ? 1 : -1);
      const bodyGuideStart = { x: pose.anchor.x + inwardDx * 20, y: pose.anchor.y };
      const bodyGuideEnd = { x: pose.anchor.x + inwardDx * 4, y: pose.anchor.y };

      return `
        <div class="leg-preview-shell outer">
          <div class="leg-preview-top">
            <strong>Side view</strong>
            <span>${onlineCount}/3</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 220 116" aria-label="${meta.label} side-view live pose">
            <path d='M ${bodyGuideStart.x.toFixed(1)} ${bodyGuideStart.y.toFixed(1)} L ${bodyGuideEnd.x.toFixed(1)} ${bodyGuideEnd.y.toFixed(1)}'
              fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='10' stroke-linecap='round' />
            <path d='M ${pose.anchor.x} ${pose.anchor.y} L ${pose.coxaEnd.x} ${pose.coxaEnd.y} L ${pose.femurEnd.x.toFixed(1)} ${pose.femurEnd.y.toFixed(1)} L ${pose.tibiaEnd.x.toFixed(1)} ${pose.tibiaEnd.y.toFixed(1)}'
              fill='none' stroke='${stroke}' stroke-width='9' stroke-linecap='round' stroke-linejoin='round' />
            <circle cx='${pose.anchor.x}' cy='${pose.anchor.y}' r='6.5' fill='#eef3f7' />
            <circle cx='${pose.coxaEnd.x}' cy='${pose.coxaEnd.y}' r='5.5' fill='#d9e2ec' />
            <circle cx='${pose.femurEnd.x.toFixed(1)}' cy='${pose.femurEnd.y.toFixed(1)}' r='5.2' fill='#c8d3de' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='5.2' fill='${stroke}' />
          </svg>
        </div>
      `;
    }

    function renderLegPreviewRow(legKey, servos) {
      const meta = LEG_META[legKey];
      const centerPreview = renderLegBirdPreview(legKey, servos);
      const outerPreview = renderLegSidePreview(legKey, servos);
      return `
        <div class="leg-preview-row">
          ${meta.side === 'left' ? `${outerPreview}${centerPreview}` : `${centerPreview}${outerPreview}`}
        </div>
      `;
    }

    function updateBadge(ok, text) {
      const badge = document.getElementById("status-badge");
      badge.textContent = text;
      badge.classList.remove("ok", "bad");
      badge.classList.add(ok ? "ok" : "bad");
    }

    async function refresh() {
      try {
        const response = await fetch(stateUrl, { cache: "no-store" });
        if (!response.ok) throw new Error(`state fetch failed: ${response.status}`);
        const state = await response.json();
        window.__latestState = state;
        window.__legPreviews = state.leg_previews ?? [];

        document.getElementById("deployment-profile").textContent = state.deployment_profile;
        document.getElementById("compute-target").textContent = state.compute_target;
        document.getElementById("servo-count").textContent = `${state.online_servo_count} / ${state.servos.length}`;
        document.getElementById("serial-port").textContent = state.serial_port;
        document.getElementById("serial-note").textContent = state.last_poll_error ?? "All configured servos replied on the last poll.";
        document.getElementById("camera-backend").textContent = state.camera_backend;
        document.getElementById("camera-note").textContent = state.camera_device ?? state.camera_pipeline;
        document.getElementById("camera-meta").textContent = state.camera_pipeline;
        document.getElementById("motion-mode").textContent = state.motion_mode ?? "-";
        document.getElementById("motion-summary").textContent = state.motion_summary ?? "-";
        document.getElementById("safety-status").textContent = state.motion_fault ? "tripped" : (state.safety_status ?? "ok");
        document.getElementById("motion-fault").textContent = state.motion_fault ?? "No safety trips latched.";
        document.getElementById("updated-at").textContent = state.updated_at_ms ? new Date(state.updated_at_ms).toLocaleTimeString() : "never";
        updateMotionButtons(state.motion_mode);
        updateImuPanel(state.imu);
        updateManualPanel(state.manual);
        updateCalibrationPanel(state.calibration);

        const faulted = state.servos.filter((servo) => servo.telemetry && servo.telemetry.faults.length > 0).length;
        const groupedServos = groupServosByLeg(state.servos);
        const liveLegs = LEG_ORDER.filter((legKey) => groupedServos[legKey].filter((servo) => servo.online).length === 3).length;
        document.getElementById("fault-summary").textContent = `${liveLegs}/${LEG_ORDER.length} legs fully live · ${faulted} servo(s) reporting status flags`;
        document.getElementById("robot-note").textContent =
          `${state.motion_mode}: ${state.motion_summary} ${state.online_servo_count}/${state.servos.length} joints responding.`;
        document.getElementById("servo-map-legs").innerHTML = renderServoMap(state.servos);

        updateBadge(
          state.online_servo_count > 0 && !state.motion_fault,
          `${state.robot_name}: ${state.motion_mode}, ${state.online_servo_count}/${state.servos.length} online`
        );

        if (state.camera_backend === "v4l2" && !streamStarted) {
          const img = document.getElementById("camera-stream");
          document.getElementById("stream-placeholder").hidden = true;
          img.hidden = false;
          img.src = cameraUrl;
          streamStarted = true;
        }

        if (state.camera_backend !== "v4l2") {
          document.getElementById("stream-placeholder").textContent =
            "This dashboard currently serves live video for the host-usb V4L2 camera path. The onboard Jetson profile is prepared, but its stream route still needs a Jetson-native capture backend.";
        }
      } catch (error) {
        updateBadge(false, "dashboard fetch error");
        document.getElementById("serial-note").textContent = String(error);
      }
    }

    refresh();
    setInterval(refresh, 500);
  </script>
</body>
</html>
"#;
