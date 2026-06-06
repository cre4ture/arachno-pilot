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
      max-width: 1480px;
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
      justify-content: space-between;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 10px;
    }

    .slider-top strong {
      font-size: 1rem;
    }

    .slider-top span {
      color: var(--accent);
      font-weight: 700;
    }

    .slider-field input[type="range"] {
      width: 100%;
      accent-color: var(--accent);
      margin: 0;
    }

    .slider-legend {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      color: var(--muted);
      font-size: 0.9rem;
      margin-top: 10px;
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

    .leg-schematic-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
    }

    .leg-schematic-card {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .leg-schematic-top {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 12px;
    }

    .leg-schematic-top strong {
      font-size: 1rem;
    }

    .leg-schematic-top span {
      color: var(--muted);
      font-size: 0.9rem;
    }

    .leg-schematic-svg {
      width: 100%;
      height: 10rem;
      display: block;
    }

    .leg-schematic-svg.mirror {
      transform: scaleX(-1);
    }

    .leg-schematic-metrics {
      margin-top: 10px;
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 8px;
      color: var(--muted);
      font-size: 0.86rem;
    }

    .leg-schematic-metrics strong {
      display: block;
      color: var(--text);
      font-size: 0.95rem;
      margin-bottom: 3px;
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
      min-width: 980px;
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
      width: 18rem;
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
      padding: 0 1.4rem;
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
      width: 20rem;
    }

    .leg-cluster::after {
      content: "";
      position: absolute;
      top: 50%;
      height: 2px;
      background: linear-gradient(90deg, rgba(255,255,255,0.08), rgba(255, 146, 84, 0.3));
    }

    .leg-cluster.left::after {
      right: -2.2rem;
      width: 2.2rem;
    }

    .leg-cluster.right::after {
      left: -2.2rem;
      width: 2.2rem;
      background: linear-gradient(90deg, rgba(255, 146, 84, 0.3), rgba(255,255,255,0.08));
    }

    .leg-cluster.front-left { top: 6%; left: 3%; }
    .leg-cluster.middle-left { top: 50%; left: 1.4%; transform: translateY(-50%); }
    .leg-cluster.rear-left { bottom: 6%; left: 3%; }
    .leg-cluster.front-right { top: 6%; right: 3%; }
    .leg-cluster.middle-right { top: 50%; right: 1.4%; transform: translateY(-50%); }
    .leg-cluster.rear-right { bottom: 6%; right: 3%; }

    .leg-name {
      margin-bottom: 10px;
      font-size: 0.78rem;
      letter-spacing: 0.16em;
      text-transform: uppercase;
      color: rgba(255,255,255,0.54);
    }

    .leg-chain {
      display: flex;
      gap: 10px;
      align-items: stretch;
    }

    .leg-chain.reverse {
      flex-direction: row-reverse;
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
        <h2>Manual Control</h2>
        <div class="muted" id="manual-summary">manual control disabled</div>
      </div>
      <div class="panel-body">
        <div class="manual-grid">
          <div class="manual-card">
            <div class="stat-label">Leg Group</div>
            <select id="manual-group"></select>
            <div class="stat-note" id="manual-group-note">Choose a leg group, then apply semantic angle offsets in degrees.</div>
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
              <span id="manual-coxa-value">0.0°</span>
            </div>
            <input id="manual-coxa-slider" type="range" min="-180" max="180" step="0.5" value="0" />
            <div class="slider-legend">
              <span id="manual-coxa-negative">back</span>
              <span id="manual-coxa-positive">forward</span>
            </div>
          </label>

          <label class="slider-field">
            <div class="slider-top">
              <strong id="manual-femur-label">Femur</strong>
              <span id="manual-femur-value">0.0°</span>
            </div>
            <input id="manual-femur-slider" type="range" min="-180" max="180" step="0.5" value="0" />
            <div class="slider-legend">
              <span id="manual-femur-negative">down</span>
              <span id="manual-femur-positive">up</span>
            </div>
          </label>

          <label class="slider-field">
            <div class="slider-top">
              <strong id="manual-tibia-label">Tibia</strong>
              <span id="manual-tibia-value">0.0°</span>
            </div>
            <input id="manual-tibia-slider" type="range" min="-180" max="180" step="0.5" value="0" />
            <div class="slider-legend">
              <span id="manual-tibia-negative">down</span>
              <span id="manual-tibia-positive">up</span>
            </div>
          </label>
        </div>

        <label class="manual-live-toggle">
          <input id="manual-live-apply" type="checkbox" />
          <span>Apply slider movement immediately while dragging</span>
        </label>

        <div class="manual-actions">
          <button id="manual-apply" type="button">Apply To Selected Group</button>
          <button id="manual-reset-group" type="button">Reset Selected Group</button>
          <button id="manual-reset-all" class="wide" type="button">Reset All Legs To Manual Zero</button>
          <button id="manual-capture" class="wide" type="button">Capture Current Pose As Manual Zero</button>
          <button id="copy-current-pose" class="wide" type="button">Copy Current Pose To Clipboard</button>
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Leg Schematic</h2>
        <div class="muted" id="leg-schematic-summary">waiting for live servo feedback</div>
      </div>
      <div class="panel-body">
        <div class="leg-schematic-grid" id="leg-schematic-grid"></div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Servos</h2>
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
                  <div class="robot-body-note" id="robot-note">Coxa, femur, tibia are drawn from the body outward.</div>
                </div>
              </div>
              <div id="servo-map-legs"></div>
            </div>
          </div>
          <div class="servo-orientation">
            The map follows the robot's physical layout. Left legs are arms 1-3 from front to back, right legs are arms 4-6, and each leg is drawn from inside to outside as coxa, femur, tibia.
          </div>
        </div>
      </div>
    </section>
  </div>

  <script>
    const stateUrl = "/api/state";
    const cameraUrl = "/camera.mjpg";
    const manualApplyUrl = "/api/manual/apply";
    const manualResetUrl = "/api/manual/reset";
    const manualCaptureUrl = "/api/manual/capture";
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
    const LEG_SCHEMATIC_ORDER = [
      "front_left",
      "front_right",
      "middle_left",
      "middle_right",
      "rear_left",
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

    function updateSliderReadout(axis) {
      document.getElementById(`manual-${axis}-value`).textContent = `${sliderValue(axis).toFixed(1)}°`;
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
        if (!slider) continue;
        slider.min = String(joint.min_deg);
        slider.max = String(joint.max_deg);
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
        : "Choose a leg group, then apply semantic angle offsets in degrees.";
    }

    function currentManualGroupValue() {
      const groupKey = document.getElementById("manual-group").value;
      return (window.__manualGroupValues ?? []).find((group) => group.key === groupKey) ?? null;
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
      document.getElementById("manual-live-apply").disabled = !enabled;
      for (const axis of MANUAL_JOINT_KEYS) {
        document.getElementById(`manual-${axis}-slider`).disabled = !enabled;
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

    function bindManualControls() {
      if (window.__manualControlsBound) return;
      window.__manualControlsBound = true;
      for (const axis of MANUAL_JOINT_KEYS) {
        document.getElementById(`manual-${axis}-slider`).addEventListener("input", () => {
          updateSliderReadout(axis);
          scheduleLiveManualApply();
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
            ? "Manual zero is captured. Sliders apply semantic angles relative to that pose."
            : "Waiting for full servo feedback or press capture once all servos are online.")
        : "Start arachno-brain with --mode manual to enable dashboard-based servo control.";

      setManualControlsEnabled(Boolean(manual?.enabled && manualGroupsReady));
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

      return `
        <article class="${classes.join(" ")}">
          <div class="servo-node-top">
            <div>
              <div class="joint-name">${jointLabel}</div>
              <div class="servo-node-id">${servo.servo_id}</div>
              <div class="servo-node-pos">${servo.position_deg != null ? `${fmt(servo.position_deg, 1)}°` : "n/a"}</div>
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

      return `
        <section class="leg-cluster ${meta.placement}">
          <div class="leg-name">${meta.label}</div>
          <div class="${chainClass}">
            ${sorted.map(renderServoNode).join("")}
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

    function legSchematicPose(servos) {
      const byJoint = Object.fromEntries(servos.map((servo) => [jointIndexForServo(servo), servo]));
      const coxa = clamp(byJoint[1]?.semantic_angle_deg ?? 0, -140, 140);
      const femur = clamp(byJoint[2]?.semantic_angle_deg ?? 0, -140, 140);
      const tibia = clamp(byJoint[3]?.semantic_angle_deg ?? 0, -140, 140);
      const anchor = { x: 36, y: 86 };
      const coxaLen = 42;
      const femurLen = 40;
      const tibiaLen = 46;

      const coxaRad = (-coxa * 0.55) * Math.PI / 180;
      const femurRad = coxaRad + ((40 - femur * 0.42) * Math.PI / 180);
      const tibiaRad = femurRad + ((46 - tibia * 0.38) * Math.PI / 180);

      const coxaEnd = pointFrom(anchor, coxaLen, coxaRad);
      const femurEnd = pointFrom(coxaEnd, femurLen, femurRad);
      const tibiaEnd = pointFrom(femurEnd, tibiaLen, tibiaRad);

      return { anchor, coxaEnd, femurEnd, tibiaEnd, coxa, femur, tibia };
    }

    function renderLegSchematicCard(legKey, servos) {
      const meta = LEG_META[legKey];
      const pose = legSchematicPose(servos);
      const onlineCount = servos.filter((servo) => servo.online).length;
      const stroke = onlineCount === 3 ? '#ff9254' : (onlineCount > 0 ? '#ffc26b' : '#5a6775');
      const fill = onlineCount === 3 ? 'rgba(255,146,84,0.16)' : 'rgba(255,255,255,0.05)';
      const mirror = meta.side === "left" ? "mirror" : "";

      return `
        <article class="leg-schematic-card">
          <div class="leg-schematic-top">
            <strong>${meta.label}</strong>
            <span>${onlineCount}/3 online</span>
          </div>
          <svg class="leg-schematic-svg ${mirror}" viewBox="0 0 240 170" aria-label="${meta.label} live pose">
            <rect x='10' y='24' width='30' height='124' rx='14' fill='${fill}' stroke='rgba(255,255,255,0.08)' />
            <path d='M ${pose.anchor.x} ${pose.anchor.y} L ${pose.coxaEnd.x.toFixed(1)} ${pose.coxaEnd.y.toFixed(1)} L ${pose.femurEnd.x.toFixed(1)} ${pose.femurEnd.y.toFixed(1)} L ${pose.tibiaEnd.x.toFixed(1)} ${pose.tibiaEnd.y.toFixed(1)}'
              fill='none' stroke='${stroke}' stroke-width='9' stroke-linecap='round' stroke-linejoin='round' />
            <circle cx='${pose.anchor.x}' cy='${pose.anchor.y}' r='6.5' fill='#eef3f7' />
            <circle cx='${pose.coxaEnd.x.toFixed(1)}' cy='${pose.coxaEnd.y.toFixed(1)}' r='5.5' fill='#d9e2ec' />
            <circle cx='${pose.femurEnd.x.toFixed(1)}' cy='${pose.femurEnd.y.toFixed(1)}' r='5.2' fill='#c8d3de' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='5.2' fill='${stroke}' />
          </svg>
          <div class="leg-schematic-metrics">
            <div><strong>${fmt(pose.coxa, 1)}°</strong>coxa</div>
            <div><strong>${fmt(pose.femur, 1)}°</strong>femur</div>
            <div><strong>${fmt(pose.tibia, 1)}°</strong>tibia</div>
          </div>
        </article>
      `;
    }

    function renderLegSchematicGrid(servos) {
      const grouped = groupServosByLeg(servos);
      return LEG_SCHEMATIC_ORDER
        .map((legKey) => renderLegSchematicCard(legKey, grouped[legKey]))
        .join("");
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
        updateImuPanel(state.imu);
        updateManualPanel(state.manual);

        const faulted = state.servos.filter((servo) => servo.telemetry && servo.telemetry.faults.length > 0).length;
        const groupedServos = groupServosByLeg(state.servos);
        const liveLegs = LEG_ORDER.filter((legKey) => groupedServos[legKey].filter((servo) => servo.online).length === 3).length;
        document.getElementById("fault-summary").textContent = `${faulted} servo(s) reporting status flags`;
        document.getElementById("leg-schematic-summary").textContent = `${liveLegs}/${LEG_ORDER.length} legs fully live`;
        document.getElementById("robot-note").textContent =
          `${state.motion_mode}: ${state.motion_summary} ${state.online_servo_count}/${state.servos.length} joints responding.`;
        document.getElementById("leg-schematic-grid").innerHTML = renderLegSchematicGrid(state.servos);
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
