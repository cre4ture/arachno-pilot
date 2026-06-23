// Shared built-in dashboard page served directly by `arachno-brain` when
// the `--dashboard` command-line option is enabled.
pub const DASHBOARD_HTML: &str = r##"<!doctype html>
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
      grid-template-columns: minmax(0, 1fr) clamp(38rem, 46vw, 52rem);
      gap: 18px;
      align-items: start;
    }

    .main-column {
      min-width: 0;
      display: grid;
      gap: 18px;
    }

    .side-rail {
      position: sticky;
      top: 24px;
      align-self: start;
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 18px;
      max-height: calc(100vh - 48px);
      overflow-y: auto;
      overflow-x: hidden;
      padding-right: 4px;
    }

    .rail-span-2 {
      grid-column: 1 / -1;
    }

    .side-rail::-webkit-scrollbar {
      width: 10px;
    }

    .side-rail::-webkit-scrollbar-thumb {
      background: rgba(255, 255, 255, 0.14);
      border-radius: 999px;
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

    .rail-panel .panel-header {
      padding: 16px 18px 0;
    }

    .rail-panel .panel-header h2 {
      font-size: 1rem;
    }

    .rail-panel .panel-body {
      padding: 16px 18px 18px;
    }

    .rail-panel {
      min-width: 0;
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

    .rail-stream .stream-shell {
      min-height: 0;
      aspect-ratio: 16 / 10;
    }

    .rail-stream .stream-shell img {
      height: 100%;
      object-fit: cover;
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

    .rail-stats {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
    }

    .rail-stat {
      min-width: 0;
      padding: 12px;
      background: var(--panel-strong);
      border-radius: 14px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .rail-stat-label {
      color: var(--muted);
      font-size: 0.72rem;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 6px;
    }

    .rail-stat-value {
      font-size: 1rem;
      font-weight: 700;
      line-height: 1.25;
      word-break: break-word;
    }

    .rail-stat-note {
      color: var(--muted);
      font-size: 0.8rem;
      line-height: 1.35;
      margin-top: 6px;
    }

    .rail-leg-grid {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 4.75rem minmax(0, 1fr);
      grid-template-areas:
        "front-left body front-right"
        "middle-left body middle-right"
        "rear-left body rear-right";
      gap: 10px 12px;
      align-items: stretch;
    }

    .rail-leg-body {
      grid-area: body;
      min-width: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 12px 8px;
      border-radius: 20px;
      border: 1px solid rgba(255,255,255,0.08);
      background:
        linear-gradient(180deg, rgba(255,255,255,0.04), rgba(0,0,0,0.18)),
        rgba(9, 13, 18, 0.7);
    }

    .rail-leg-axis {
      color: rgba(255,255,255,0.44);
      font-size: 0.68rem;
      letter-spacing: 0.12em;
      text-transform: uppercase;
    }

    .rail-leg-core {
      position: relative;
      flex: 1;
      width: 100%;
      min-height: 100%;
      border-radius: 999px;
      clip-path: polygon(20% 0%, 80% 0%, 100% 18%, 100% 82%, 80% 100%, 20% 100%, 0% 82%, 0% 18%);
      background:
        radial-gradient(circle at top, rgba(255,255,255,0.09), transparent 52%),
        linear-gradient(180deg, rgba(255, 146, 84, 0.18), rgba(255, 146, 84, 0.04));
      border: 1px solid rgba(255,255,255,0.1);
      box-shadow:
        inset 0 1px 0 rgba(255,255,255,0.1),
        0 10px 22px rgba(0,0,0,0.18);
    }

    .rail-leg-side {
      position: absolute;
      top: 50%;
      color: rgba(255,255,255,0.36);
      font-size: 0.66rem;
      letter-spacing: 0.12em;
      text-transform: uppercase;
    }

    .rail-leg-side.left {
      left: -0.2rem;
      transform: translate(-50%, -50%) rotate(-90deg);
    }

    .rail-leg-side.right {
      right: -0.2rem;
      transform: translate(50%, -50%) rotate(90deg);
    }

    .rail-leg-card {
      min-width: 0;
      padding: 12px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .rail-leg-card.live {
      border-color: rgba(101, 214, 164, 0.2);
    }

    .rail-leg-top {
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: baseline;
      margin-bottom: 8px;
    }

    .rail-leg-name {
      font-size: 0.82rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: rgba(255,255,255,0.84);
    }

    .rail-leg-count {
      color: var(--muted);
      font-size: 0.78rem;
      white-space: nowrap;
    }

    .rail-leg-previews {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 6px;
    }

    .rail-leg-card.front-left { grid-area: front-left; }
    .rail-leg-card.middle-left { grid-area: middle-left; }
    .rail-leg-card.rear-left { grid-area: rear-left; }
    .rail-leg-card.front-right { grid-area: front-right; }
    .rail-leg-card.middle-right { grid-area: middle-right; }
    .rail-leg-card.rear-right { grid-area: rear-right; }

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

    .slider-aux-row {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      align-items: end;
      margin-top: 12px;
    }

    .slider-inline-control {
      min-width: 10rem;
      display: grid;
      gap: 6px;
    }

    .slider-inline-label {
      color: var(--muted);
      font-size: 0.75rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .slider-inline-control input {
      width: 100%;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(7, 11, 16, 0.88);
      color: var(--text);
      border-radius: 10px;
      padding: 8px 10px;
      font: inherit;
    }

    .slider-aux-note {
      flex: 1 1 18rem;
      color: var(--muted);
      font-size: 0.84rem;
      line-height: 1.45;
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

    .leg-preview-shell.compact {
      padding: 8px 8px 6px;
      border-radius: 12px;
    }

    .leg-preview-shell.compact .leg-preview-top {
      margin-bottom: 5px;
      font-size: 0.68rem;
    }

    .leg-preview-shell.compact .leg-preview-svg {
      height: 3.8rem;
    }

    .robot-scene-card {
      margin-top: 0;
      padding: 14px;
      border-radius: 18px;
      border: 1px solid rgba(255,255,255,0.08);
      background:
        radial-gradient(circle at top left, rgba(125, 200, 255, 0.12), transparent 52%),
        linear-gradient(180deg, rgba(255,255,255,0.05), rgba(0,0,0,0.22));
    }

    .robot-scene-top {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 10px;
      color: var(--muted);
      font-size: 0.82rem;
    }

    .robot-scene-shell {
      position: relative;
      min-height: 22.4rem;
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.08);
      background:
        linear-gradient(180deg, rgba(12, 18, 24, 0.92), rgba(6, 9, 13, 0.96));
      overflow: hidden;
      isolation: isolate;
    }

    .robot-scene-canvas {
      width: 100%;
      height: 22.4rem;
      display: block;
    }

    .robot-scene-canvas[hidden] {
      display: none;
    }

    .robot-scene-canvas canvas {
      width: 100%;
      height: 100%;
      display: block;
      touch-action: none;
    }

    .robot-scene-empty {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      padding: 16px;
      text-align: center;
      color: rgba(214, 224, 235, 0.82);
      background:
        radial-gradient(circle at top, rgba(125, 200, 255, 0.08), transparent 48%),
        linear-gradient(180deg, rgba(12, 18, 24, 0.72), rgba(6, 9, 13, 0.84));
      z-index: 2;
    }

    .robot-scene-empty[hidden] {
      display: none;
    }

    .robot-scene-hud {
      position: absolute;
      left: 12px;
      right: 12px;
      bottom: 12px;
      display: flex;
      justify-content: space-between;
      gap: 10px;
      align-items: center;
      pointer-events: none;
      z-index: 3;
      color: rgba(214, 224, 235, 0.78);
      font-size: 0.74rem;
      line-height: 1.35;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }

    .robot-scene-hint,
    .robot-scene-axes {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 10px;
      border-radius: 999px;
      background: rgba(6, 9, 13, 0.46);
      border: 1px solid rgba(255,255,255,0.08);
      backdrop-filter: blur(10px);
    }

    .robot-scene-axis-dot {
      width: 8px;
      height: 8px;
      border-radius: 999px;
      display: inline-block;
    }

    .robot-scene-axis-dot.front { background: #ff9254; }
    .robot-scene-axis-dot.left { background: #7dc8ff; }
    .robot-scene-axis-dot.up { background: #9cf0a8; }

    .robot-scene-note {
      margin-top: 10px;
      color: rgba(214, 224, 235, 0.72);
      font-size: 0.82rem;
      line-height: 1.45;
    }

    .robot-scene-feedback {
      margin-top: 10px;
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 8px;
    }

    .robot-scene-feedback-chip {
      padding: 9px 10px;
      border-radius: 12px;
      border: 1px solid rgba(255,255,255,0.08);
      background: rgba(10, 14, 20, 0.58);
      min-width: 0;
    }

    .robot-scene-feedback-chip.high {
      border-color: rgba(255, 111, 97, 0.30);
      background: rgba(61, 19, 20, 0.46);
    }

    .robot-scene-feedback-chip.medium {
      border-color: rgba(255, 194, 107, 0.26);
      background: rgba(54, 35, 10, 0.42);
    }

    .robot-scene-feedback-chip.low {
      border-color: rgba(101, 214, 164, 0.22);
      background: rgba(14, 42, 31, 0.40);
    }

    .robot-scene-feedback-top {
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: baseline;
      margin-bottom: 4px;
      color: rgba(238, 243, 247, 0.88);
      font-size: 0.78rem;
    }

    .robot-scene-feedback-name {
      font-weight: 600;
      letter-spacing: 0.03em;
    }

    .robot-scene-feedback-load {
      color: rgba(255,255,255,0.72);
      font-variant-numeric: tabular-nums;
    }

    .robot-scene-feedback-meta {
      color: rgba(148, 164, 182, 0.96);
      font-size: 0.73rem;
      line-height: 1.4;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .rail-visual-card {
      display: grid;
      gap: 12px;
    }

    .rail-tab-strip {
      display: inline-flex;
      gap: 8px;
      align-items: center;
      padding: 4px;
      border-radius: 999px;
      background: rgba(10, 14, 20, 0.68);
      border: 1px solid rgba(255,255,255,0.08);
      width: fit-content;
    }

    .rail-tab-btn {
      appearance: none;
      border: 0;
      background: transparent;
      color: var(--muted);
      border-radius: 999px;
      padding: 8px 14px;
      font: inherit;
      font-size: 0.8rem;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      cursor: pointer;
      transition: background 140ms ease, color 140ms ease, transform 140ms ease;
    }

    .rail-tab-btn:hover {
      color: var(--text);
    }

    .rail-tab-btn.active {
      background: rgba(255, 146, 84, 0.16);
      color: #ffd7c0;
      box-shadow: inset 0 0 0 1px rgba(255, 146, 84, 0.18);
    }

    .rail-tab-pane[hidden] {
      display: none;
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

    .arm-servo-chain {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
      gap: 10px;
      margin-top: 16px;
    }

    .muted {
      color: var(--muted);
    }

    @media (max-width: 980px) {
      .layout { grid-template-columns: 1fr; }
      .side-rail {
        position: static;
        grid-template-columns: 1fr;
        max-height: none;
        overflow: visible;
        padding-right: 0;
        order: -1;
      }
      .stats { grid-template-columns: 1fr; }
      .page { padding: 18px; }
    }

    @media (max-width: 720px) {
      .rail-stats {
        grid-template-columns: 1fr;
      }

      .rail-leg-grid {
        grid-template-columns: 1fr;
        grid-template-areas: none;
      }

      .rail-leg-body {
        display: none;
      }

      .rail-leg-card {
        grid-area: auto !important;
      }
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
      <div class="main-column">
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

    <section class="panel">
      <div class="panel-header">
        <h2>Motion Commands</h2>
        <div class="muted" id="motion-cmd-summary">ready</div>
      </div>
      <div class="panel-body">
        <div class="motion-cmd-grid">
          <button class="motion-btn" id="btn-manual" type="button" data-cmd="manual">Manual</button>
          <button class="motion-btn" id="btn-tilted_stand" type="button" data-cmd="tilted_stand">Tilted Stand</button>
          <button class="motion-btn" id="btn-stand_up" type="button" data-cmd="stand_up">Stand Up</button>
          <button class="motion-btn" id="btn-stand_up_high" type="button" data-cmd="stand_up_high">Stand Up High</button>
          <button class="motion-btn" id="btn-stand_high" type="button" data-cmd="stand_high">Stand High</button>
          <button class="motion-btn" id="btn-lay_down" type="button" data-cmd="lay_down">Lay Down</button>
          <button class="motion-btn" id="btn-sit_down" type="button" data-cmd="sit_down">Sit Down</button>
          <button class="motion-btn" id="btn-stand" type="button" data-cmd="stand">Stand</button>
          <button class="motion-btn" id="btn-stop" type="button" data-cmd="stop">Stop</button>
          <button class="motion-btn" id="btn-walk_forward" type="button" data-cmd="walk_forward">Walk Forward</button>
          <button class="motion-btn" id="btn-walk_forward_high" type="button" data-cmd="walk_forward_high">Walk Forward High</button>
          <button class="motion-btn" id="btn-walk_backward" type="button" data-cmd="walk_backward">Walk Backward</button>
          <button class="motion-btn" id="btn-walk_backward_high" type="button" data-cmd="walk_backward_high">Walk Backward High</button>
          <button class="motion-btn" id="btn-sidewalk_left" type="button" data-cmd="sidewalk_left">Sidewalk Left</button>
          <button class="motion-btn" id="btn-sidewalk_left_high" type="button" data-cmd="sidewalk_left_high">Sidewalk Left High</button>
          <button class="motion-btn" id="btn-sidewalk_right" type="button" data-cmd="sidewalk_right">Sidewalk Right</button>
          <button class="motion-btn" id="btn-sidewalk_right_high" type="button" data-cmd="sidewalk_right_high">Sidewalk Right High</button>
          <button class="motion-btn" id="btn-rotate_left" type="button" data-cmd="rotate_left">Rotate Left</button>
          <button class="motion-btn" id="btn-rotate_right" type="button" data-cmd="rotate_right">Rotate Right</button>
        </div>
        <div class="stat-note" id="motion-cmd-note" style="margin-top: 12px;">
          Commands switch the brain mode immediately. High-step walk modes target roughly 10 cm of foot clearance for small hurdles. The active mode is highlighted. Safety faults clear on any mode switch.
        </div>
      </div>
    </section>

    <section class="panel">
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

    <section class="panel">
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
                  <div class="robot-body-note" id="robot-note">Bird's-eye leg previews follow the ROS body frame: +x forward and +y left.</div>
                </div>
              </div>
              <div id="servo-map-legs"></div>
            </div>
          </div>
          <div class="servo-orientation">
            The map follows the robot's physical layout in the ROS body frame. Left legs are arms 1-3 from front to back, right legs are arms 4-6. Each cluster combines a top-view live leg preview with the detailed coxa, femur, and tibia telemetry cards.
          </div>
        </div>
      </div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h2>Arm</h2>
        <div class="muted" id="arm-summary">arm unavailable</div>
      </div>
      <div class="panel-body">
        <div class="manual-grid">
          <div class="manual-card">
            <div class="stat-label">Arm Bus</div>
            <div class="stat-value" id="arm-name">No arm configured</div>
            <div class="stat-note" id="arm-bus-port">Load a profile with an arm store to enable this section.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Arm Mode</div>
            <div class="stat-value" id="arm-mode-state">unconfigured</div>
            <div class="stat-note" id="arm-mode-note">Switch the motion mode to <code>Manual</code> to enable arm control when an arm is configured.</div>
          </div>
        </div>

        <div class="manual-grid" style="margin-top: 18px;">
          <div class="manual-card">
            <div class="stat-label">Mount</div>
            <div class="stat-value" id="arm-mount">-</div>
            <div class="stat-note" id="arm-servo-count-note">0 / 0 configured arm servos replying.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Bus Health</div>
            <div class="stat-value" id="arm-servo-count">0 / 0</div>
            <div class="stat-note" id="arm-bus-note">Waiting for arm bus state.</div>
          </div>
        </div>

        <div id="arm-slider-fields" class="manual-sliders">
          <div class="stat-note">No arm joints are configured for this profile.</div>
        </div>

        <label class="manual-live-toggle">
          <input id="arm-live-apply" type="checkbox" checked />
          <span>Apply arm slider movement immediately while dragging</span>
        </label>

        <div class="manual-actions">
          <button id="arm-apply" type="button">Apply Arm Pose</button>
          <button id="arm-reset" type="button">Reset Arm To Zero/Home</button>
          <button id="arm-capture" class="wide" type="button">Capture Current Pose As Arm Zero/Home</button>
          <button id="arm-sync-current" class="wide" type="button">Set Arm Target To Current Pose</button>
        </div>

        <div class="stat-note" id="arm-layout-note" style="margin-top: 8px;">Configured arm joints will appear here in arm-config order.</div>
        <div id="arm-servo-chain" class="arm-servo-chain"></div>
      </div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h2>Tilted Stand</h2>
        <div class="muted" id="tilted-stand-summary">tilted stand disabled</div>
      </div>
      <div class="panel-body">
        <div class="manual-grid">
          <div class="manual-card">
            <div class="stat-label">Tilted Stand Mode</div>
            <div class="stat-value" id="tilted-stand-mode-state">disabled</div>
            <div class="stat-note" id="tilted-stand-mode-note">Switch the motion mode to <code>Tilted Stand</code> to enable pitch and roll body-tilt control.</div>
          </div>
          <div class="manual-card">
            <div class="stat-label">Reference Stance</div>
            <div class="stat-value">Captured On Entry</div>
            <div class="stat-note">The mode uses the robot's measured stance when it arms as level zero. Leave and re-enter tilted stand to recapture a new base stance.</div>
          </div>
        </div>

        <div class="manual-sliders">
          <label class="slider-field">
            <div class="slider-top">
              <strong id="tilted-stand-pitch-label">Pitch</strong>
            </div>
            <div class="slider-main-row">
              <div class="slider-value-box">
                <input id="tilted-stand-pitch-input" type="number" min="-20" max="20" step="0.1" value="0.0" />
                <span>°</span>
              </div>
              <div class="slider-track">
                <input id="tilted-stand-pitch-slider" type="range" min="-20" max="20" step="0.5" value="0" />
                <div class="slider-legend">
                  <span id="tilted-stand-pitch-negative">nose down</span>
                  <span id="tilted-stand-pitch-positive">nose up</span>
                </div>
              </div>
            </div>
          </label>

          <label class="slider-field">
            <div class="slider-top">
              <strong id="tilted-stand-roll-label">Roll</strong>
            </div>
            <div class="slider-main-row">
              <div class="slider-value-box">
                <input id="tilted-stand-roll-input" type="number" min="-20" max="20" step="0.1" value="0.0" />
                <span>°</span>
              </div>
              <div class="slider-track">
                <input id="tilted-stand-roll-slider" type="range" min="-20" max="20" step="0.5" value="0" />
                <div class="slider-legend">
                  <span id="tilted-stand-roll-negative">right up</span>
                  <span id="tilted-stand-roll-positive">left up</span>
                </div>
              </div>
            </div>
          </label>
        </div>

        <label class="manual-live-toggle">
          <input id="tilted-stand-live-apply" type="checkbox" checked />
          <span>Apply tilt changes immediately while dragging</span>
        </label>

        <div class="manual-actions">
          <button id="tilted-stand-apply" type="button">Apply Tilt</button>
          <button id="tilted-stand-reset" type="button">Reset To Level</button>
        </div>
        <div class="stat-note" id="tilted-stand-limits-note" style="margin-top: 8px;">Pitch and roll follow the ROS body frame and are clamped to the mode's configured limits.</div>
      </div>
    </section>

    <section class="panel">
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
            <div class="stat-note" id="manual-mode-note">Switch the motion mode to <code>Manual</code> to enable dashboard-based servo control.</div>
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
            <div class="slider-aux-row">
              <div class="slider-inline-control">
                <div class="slider-inline-label">Torque Limit</div>
                <input id="manual-coxa-torque-limit" type="number" min="0" max="1000" step="1" value="1000" aria-label="Torque limit for the selected group's coxa servos" />
              </div>
              <div class="slider-aux-note">Changing this value syncs the selected group's coxa targets to the live pose before applying the new torque limit.</div>
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
            <div class="slider-aux-row">
              <div class="slider-inline-control">
                <div class="slider-inline-label">Torque Limit</div>
                <input id="manual-femur-torque-limit" type="number" min="0" max="1000" step="1" value="1000" aria-label="Torque limit for the selected group's femur servos" />
              </div>
              <div class="slider-aux-note">Changing this value syncs the selected group's femur targets to the live pose before applying the new torque limit.</div>
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
            <div class="slider-aux-row">
              <div class="slider-inline-control">
                <div class="slider-inline-label">Torque Limit</div>
                <input id="manual-tibia-torque-limit" type="number" min="0" max="1000" step="1" value="1000" aria-label="Torque limit for the selected group's tibia servos" />
              </div>
              <div class="slider-aux-note">Changing this value syncs the selected group's tibia targets to the live pose before applying the new torque limit.</div>
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

      <aside class="side-rail" aria-label="Persistent telemetry rail">
        <section class="panel rail-panel rail-stream">
          <div class="panel-header">
            <h2>Camera</h2>
            <div class="muted" id="camera-meta">starting...</div>
          </div>
          <div class="panel-body">
            <div id="stream-shell" class="stream-shell">
              <div class="stream-placeholder" id="stream-placeholder">Preparing camera stream...</div>
              <img id="camera-stream" alt="Camera stream" hidden />
            </div>
            <div class="stat-note" id="camera-rail-note" style="margin-top: 10px;">Waiting for camera details.</div>
          </div>
        </section>

        <section class="panel rail-panel">
          <div class="panel-header">
            <h2>IMU Snapshot</h2>
            <div class="muted" id="rail-imu-summary">waiting for IMU state</div>
          </div>
          <div class="panel-body">
            <div class="rail-stats">
              <div class="rail-stat">
                <div class="rail-stat-label">Attitude</div>
                <div class="rail-stat-value" id="rail-imu-attitude">-</div>
                <div class="rail-stat-note" id="rail-imu-attitude-note">-</div>
              </div>
              <div class="rail-stat">
                <div class="rail-stat-label">Motion</div>
                <div class="rail-stat-value" id="rail-imu-motion">-</div>
                <div class="rail-stat-note" id="rail-imu-motion-note">-</div>
              </div>
              <div class="rail-stat">
                <div class="rail-stat-label">Bridge</div>
                <div class="rail-stat-value" id="rail-imu-mode">-</div>
                <div class="rail-stat-note" id="rail-imu-device">-</div>
              </div>
              <div class="rail-stat">
                <div class="rail-stat-label">Sensor</div>
                <div class="rail-stat-value" id="rail-imu-sensor-kind">-</div>
                <div class="rail-stat-note" id="rail-imu-sensor-note">-</div>
              </div>
            </div>
          </div>
        </section>

        <section class="panel rail-panel rail-span-2">
          <div class="panel-header">
            <h2>Leg Visuals</h2>
            <div class="muted" id="rail-leg-summary">waiting for leg telemetry</div>
          </div>
          <div class="panel-body">
            <div class="rail-visual-card">
              <div class="rail-tab-strip" role="tablist" aria-label="Leg visual mode">
                <button id="rail-visual-tab-legs" class="rail-tab-btn" type="button" role="tab" aria-selected="false" aria-controls="rail-visual-pane-legs">Leg Glance</button>
                <button id="rail-visual-tab-body" class="rail-tab-btn active" type="button" role="tab" aria-selected="true" aria-controls="rail-visual-pane-body">3D Body</button>
              </div>
              <div id="rail-visual-pane-legs" class="rail-tab-pane" role="tabpanel" aria-labelledby="rail-visual-tab-legs" hidden>
                <div id="rail-leg-previews" class="rail-leg-grid">
                  <div class="stat-note">Waiting for live leg previews.</div>
                </div>
              </div>
              <div id="rail-visual-pane-body" class="rail-tab-pane" role="tabpanel" aria-labelledby="rail-visual-tab-body">
                <div class="robot-scene-card">
                  <div class="robot-scene-top">
                    <strong>3D Body View</strong>
                    <span id="robot-scene-summary">ROS body frame</span>
                  </div>
                  <div id="robot-scene-view" class="robot-scene-shell">
                    <div id="robot-scene-canvas" class="robot-scene-canvas" aria-label="Interactive 3D body view in ROS body coordinates" hidden></div>
                    <div id="robot-scene-empty" class="robot-scene-empty">Waiting for live body geometry.</div>
                    <div class="robot-scene-hud">
                      <span class="robot-scene-hint">drag to orbit · scroll to zoom · double-click to reset</span>
                      <span class="robot-scene-axes">
                        <span class="robot-scene-axis-dot front"></span>+x front
                        <span class="robot-scene-axis-dot left"></span>+y left
                        <span class="robot-scene-axis-dot up"></span>+z up
                      </span>
                    </div>
                  </div>
                  <div class="robot-scene-note" id="robot-scene-note">
                    The view uses a WebGL renderer in the ROS body frame: +x forward, +y left, +z up. The chassis shell is still a nominal layout guide until a full body model is configured.
                  </div>
                  <div id="robot-scene-feedback" class="robot-scene-feedback">
                    <div class="stat-note">Waiting for live servo load feedback.</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>
      </aside>
    </section>
  </div>

  <script type="importmap">
    {
      "imports": {
        "three": "https://unpkg.com/three@0.166.1/build/three.module.js",
        "three/addons/": "https://unpkg.com/three@0.166.1/examples/jsm/"
      }
    }
  </script>

  <script type="module">
    const stateUrl = "/api/state";
    const cameraUrl = "/camera.mjpg";
    const motionCommandUrl = "/api/motion/command";
    const manualApplyUrl = "/api/manual/apply";
    const manualResetUrl = "/api/manual/reset";
    const manualCaptureUrl = "/api/manual/capture";
    const manualTorqueLimitUrl = "/api/manual/torque-limit";
    const manualSyncCurrentUrl = "/api/manual/sync-current";
    const manualJumpUrl = "/api/manual/jump";
    const armApplyUrl = "/api/arm/apply";
    const armResetUrl = "/api/arm/reset";
    const armCaptureUrl = "/api/arm/capture";
    const armSyncCurrentUrl = "/api/arm/sync-current";
    const armTorqueLimitUrl = "/api/arm/torque-limit";
    const armJumpUrl = "/api/arm/jump";
    const tiltedStandApplyUrl = "/api/tilted-stand/apply";
    const tiltedStandResetUrl = "/api/tilted-stand/reset";
    const calibrationCaptureUrl = "/api/calibration/capture";
    const calibrationClearUrl = "/api/calibration/clear";
    const calibrationReloadUrl = "/api/calibration/reload";
    const manualLiveApplyIntervalMs = 200;
    const armLiveApplyIntervalMs = 200;
    const tiltedStandLiveApplyIntervalMs = 200;
    let streamStarted = false;
    let manualGroupsReady = false;
    let manualLiveApplyTimer = null;
    let manualLiveApplyPending = false;
    let lastManualLiveApplyAt = 0;
    let manualSlidersInitialized = { value: false };
    let armLiveApplyTimer = null;
    let armLiveApplyPending = false;
    let lastArmLiveApplyAt = 0;
    let armSlidersInitialized = { value: false };
    let armTorqueSyncPending = false;
    let robotSceneRuntime = null;
    let tiltedStandLiveApplyTimer = null;
    let tiltedStandLiveApplyPending = false;
    let lastTiltedStandLiveApplyAt = 0;
    let tiltedStandSlidersInitialized = { value: false };
    const manualPanelState = { enabled: false, ready: false };
    const armPanelState = { configured: false, enabled: false, ready: false };
    const tiltedStandPanelState = { enabled: false, ready: false };
    const railVisualTabState = { active: "body" };
    let THREE_NS = null;
    let OrbitControlsCtor = null;
    let robotSceneModuleError = null;
    try {
      THREE_NS = await import("three");
      ({ OrbitControls: OrbitControlsCtor } = await import("three/addons/controls/OrbitControls.js"));
    } catch (error) {
      robotSceneModuleError = error;
      console.warn("robot scene renderer unavailable", error);
    }
    const LEG_ORDER = [
      "front_left",
      "middle_left",
      "rear_left",
      "front_right",
      "middle_right",
      "rear_right",
    ];
    const LEG_META = {
      front_left: { label: "Front left", placement: "front-left left", railClass: "front-left", side: "left" },
      middle_left: { label: "Middle left", placement: "middle-left left", railClass: "middle-left", side: "left" },
      rear_left: { label: "Rear left", placement: "rear-left left", railClass: "rear-left", side: "left" },
      front_right: { label: "Front right", placement: "front-right right", railClass: "front-right", side: "right" },
      middle_right: { label: "Middle right", placement: "middle-right right", railClass: "middle-right", side: "right" },
      rear_right: { label: "Rear right", placement: "rear-right right", railClass: "rear-right", side: "right" },
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
    const TILTED_STAND_AXES = ["pitch", "roll"];

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

    function torqueLimitValue(inputId) {
      const value = Number(document.getElementById(inputId).value);
      if (!Number.isFinite(value)) return 1000;
      return Math.min(1000, Math.max(0, Math.round(value)));
    }

    function manualTorqueLimitValue() {
      return torqueLimitValue("manual-torque-limit");
    }

    function manualAxisTorqueLimitValue(axis) {
      return torqueLimitValue(`manual-${axis}-torque-limit`);
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

    function armSliderValue(jointKey) {
      return Number(document.getElementById(`arm-${jointKey}-slider`).value);
    }

    function armJointRange(jointKey) {
      const slider = document.getElementById(`arm-${jointKey}-slider`);
      return {
        min: Number(slider.min),
        max: Number(slider.max),
      };
    }

    function clampArmJointValue(jointKey, value) {
      const { min, max } = armJointRange(jointKey);
      return Math.min(max, Math.max(min, value));
    }

    function updateArmReadout(jointKey) {
      document.getElementById(`arm-${jointKey}-input`).value = armSliderValue(jointKey).toFixed(1);
    }

    function armJumpValue(jointKey) {
      const value = Number(document.getElementById(`arm-${jointKey}-jump`).value);
      return Number.isFinite(value) ? value : 0;
    }

    function armJointTorqueLimitValue(jointKey) {
      return torqueLimitValue(`arm-${jointKey}-torque-limit`);
    }

    function armLiveApplyEnabled() {
      return document.getElementById("arm-live-apply").checked;
    }

    function tiltedStandValue(axis) {
      return Number(document.getElementById(`tilted-stand-${axis}-slider`).value);
    }

    function tiltedStandRange(axis) {
      const slider = document.getElementById(`tilted-stand-${axis}-slider`);
      return {
        min: Number(slider.min),
        max: Number(slider.max),
      };
    }

    function clampTiltedStandAxisValue(axis, value) {
      const { min, max } = tiltedStandRange(axis);
      return Math.min(max, Math.max(min, value));
    }

    function updateTiltedStandReadout(axis) {
      document.getElementById(`tilted-stand-${axis}-input`).value = tiltedStandValue(axis).toFixed(1);
    }

    function tiltedStandLiveApplyEnabled() {
      return document.getElementById("tilted-stand-live-apply").checked;
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

    function scheduleLiveArmApply() {
      if (!armLiveApplyEnabled()) return;
      armLiveApplyPending = true;
      if (armLiveApplyTimer) return;

      const now = Date.now();
      const delay = Math.max(0, armLiveApplyIntervalMs - (now - lastArmLiveApplyAt));
      armLiveApplyTimer = setTimeout(async () => {
        armLiveApplyTimer = null;
        if (!armLiveApplyPending) return;
        armLiveApplyPending = false;
        lastArmLiveApplyAt = Date.now();
        try {
          await applyArmPose();
        } catch (error) {
          document.getElementById("arm-summary").textContent = String(error);
        }
        if (armLiveApplyPending) {
          scheduleLiveArmApply();
        }
      }, delay);
    }

    function scheduleLiveTiltedStandApply() {
      if (!tiltedStandLiveApplyEnabled()) return;
      tiltedStandLiveApplyPending = true;
      if (tiltedStandLiveApplyTimer) return;

      const now = Date.now();
      const delay = Math.max(0, tiltedStandLiveApplyIntervalMs - (now - lastTiltedStandLiveApplyAt));
      tiltedStandLiveApplyTimer = setTimeout(async () => {
        tiltedStandLiveApplyTimer = null;
        if (!tiltedStandLiveApplyPending) return;
        tiltedStandLiveApplyPending = false;
        lastTiltedStandLiveApplyAt = Date.now();
        try {
          await applyTiltedStand();
        } catch (error) {
          document.getElementById("tilted-stand-summary").textContent = String(error);
        }
        if (tiltedStandLiveApplyPending) {
          scheduleLiveTiltedStandApply();
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

    function armMountLabel(mount) {
      return mount ? String(mount).replaceAll("_", " ") : "-";
    }

    function currentArmJointValue(jointKey) {
      return (window.__armJointValues ?? []).find((joint) => joint.key === jointKey) ?? null;
    }

    function renderArmSliderField(joint) {
      const note = joint.note || `${joint.axis} / ${joint.segment}`;
      return `
        <label class="slider-field">
          <div class="slider-top">
            <strong>${joint.label}</strong>
          </div>
          <div class="slider-main-row">
            <div class="slider-value-box">
              <input id="arm-${joint.key}-input" type="number" min="${joint.min_deg}" max="${joint.max_deg}" step="0.1" value="0.0" />
              <span>°</span>
            </div>
            <div class="slider-track">
              <input id="arm-${joint.key}-slider" type="range" min="${joint.min_deg}" max="${joint.max_deg}" step="0.5" value="0" />
              <div class="slider-legend">
                <span id="arm-${joint.key}-negative">${joint.negative_label}</span>
                <span id="arm-${joint.key}-positive">${joint.positive_label}</span>
              </div>
            </div>
            <div class="slider-jump">
              <input id="arm-${joint.key}-jump" type="number" step="0.1" value="5.0" aria-label="Relative ${joint.label} angle jump in degrees" />
              <button id="arm-${joint.key}-jump-apply" type="button">Jump</button>
            </div>
          </div>
          <div class="slider-aux-row">
            <div class="slider-inline-control">
              <div class="slider-inline-label">Torque Limit</div>
              <input id="arm-${joint.key}-torque-limit" type="number" min="0" max="1000" step="1" value="1000" aria-label="Torque limit for ${joint.label}" />
            </div>
            <div class="slider-aux-note">Changing this value syncs ${joint.label} to the live pose before applying the new torque limit.</div>
          </div>
          <div class="stat-note">${note}</div>
        </label>
      `;
    }

    function ensureArmSliderFields(joints) {
      const signature = JSON.stringify(joints ?? []);
      if (window.__armSliderSignature === signature) return;

      const container = document.getElementById("arm-slider-fields");
      container.innerHTML = joints?.length
        ? joints.map(renderArmSliderField).join("")
        : '<div class="stat-note">No arm joints are configured for this profile.</div>';
      window.__armSliderSignature = signature;
      window.__armControlsBound = false;
      armSlidersInitialized.value = false;
    }

    function setArmAxisFromInput(jointKey) {
      const input = document.getElementById(`arm-${jointKey}-input`);
      const slider = document.getElementById(`arm-${jointKey}-slider`);
      const value = Number(input.value);
      if (!Number.isFinite(value)) {
        updateArmReadout(jointKey);
        return;
      }
      const clamped = clampArmJointValue(jointKey, value);
      slider.value = String(clamped);
      updateArmReadout(jointKey);
    }

    function setArmSlidersFromState(force = false) {
      const joints = window.__armJoints ?? [];
      if (!joints.length) return;
      if (!force && armSlidersInitialized.value) return;
      for (const joint of joints) {
        const value = currentArmJointValue(joint.key)?.angle_deg ?? 0;
        document.getElementById(`arm-${joint.key}-slider`).value = String(value.toFixed(1));
        updateArmReadout(joint.key);
      }
      armSlidersInitialized.value = true;
    }

    function setArmControlsEnabled(enabled) {
      document.getElementById("arm-apply").disabled = !enabled;
      document.getElementById("arm-reset").disabled = !enabled;
      document.getElementById("arm-capture").disabled = !enabled;
      document.getElementById("arm-sync-current").disabled = !enabled;
      document.getElementById("arm-live-apply").disabled = !enabled;
      for (const joint of window.__armJoints ?? []) {
        document.getElementById(`arm-${joint.key}-slider`).disabled = !enabled;
        document.getElementById(`arm-${joint.key}-input`).disabled = !enabled;
        document.getElementById(`arm-${joint.key}-jump`).disabled = !enabled;
        document.getElementById(`arm-${joint.key}-jump-apply`).disabled = !enabled;
        document.getElementById(`arm-${joint.key}-torque-limit`).disabled = !enabled;
      }
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
        document.getElementById(`manual-${axis}-torque-limit`).disabled = !enabled;
      }
    }

    function setTiltedStandSlidersFromState(tiltedStand, force = false) {
      if (!tiltedStand) return;
      if (!force && tiltedStandSlidersInitialized.value) return;
      document.getElementById("tilted-stand-pitch-slider").value = String((tiltedStand.pitch_deg ?? 0).toFixed(1));
      document.getElementById("tilted-stand-roll-slider").value = String((tiltedStand.roll_deg ?? 0).toFixed(1));
      for (const axis of TILTED_STAND_AXES) {
        updateTiltedStandReadout(axis);
      }
      tiltedStandSlidersInitialized.value = true;
    }

    function setTiltedStandControlsEnabled(enabled) {
      document.getElementById("tilted-stand-apply").disabled = !enabled;
      document.getElementById("tilted-stand-reset").disabled = !enabled;
      document.getElementById("tilted-stand-live-apply").disabled = !enabled;
      for (const axis of TILTED_STAND_AXES) {
        document.getElementById(`tilted-stand-${axis}-slider`).disabled = !enabled;
        document.getElementById(`tilted-stand-${axis}-input`).disabled = !enabled;
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
        manual: "manual",
        tilted_stand: "tilted_stand",
        stand_up: "stand_up",
        stand_up_high: "stand_up_high",
        stand_high: "stand_high",
        lay_down: "lay_down",
        sit_down: "sit_down",
        stand: "stand",
        slow_walk: "walk_forward",
        slow_walk_high: "walk_forward_high",
        backward_walk: "walk_backward",
        backward_walk_high: "walk_backward_high",
        rotate_left: "rotate_left",
        rotate_right: "rotate_right",
        sidewalk_left: "sidewalk_left",
        sidewalk_left_high: "sidewalk_left_high",
        sidewalk_right: "sidewalk_right",
        sidewalk_right_high: "sidewalk_right_high",
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

    async function applyArmPose() {
      const result = await postJson(armApplyUrl, {
        joints: (window.__armJoints ?? []).map((joint) => ({
          joint_key: joint.key,
          angle_deg: armSliderValue(joint.key),
        })),
      });
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
    }

    async function resetArmPose() {
      const result = await postJson(armResetUrl, {});
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
      setArmSlidersFromState(true);
    }

    async function captureArmZero() {
      const result = await postJson(armCaptureUrl, {});
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
      setArmSlidersFromState(true);
    }

    async function syncArmTargetToCurrent() {
      const result = await postJson(armSyncCurrentUrl, {});
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
      setArmSlidersFromState(true);
    }

    async function applyArmJointJump(jointKey) {
      const result = await postJson(armJumpUrl, {
        joint_key: jointKey,
        delta_deg: armJumpValue(jointKey),
      });
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
      setArmSlidersFromState(true);
    }

    async function applyArmJointTorqueLimit(jointKey) {
      const result = await postJson(armTorqueLimitUrl, {
        joint_key: jointKey,
        torque_limit: armJointTorqueLimitValue(jointKey),
      });
      armTorqueSyncPending = true;
      document.getElementById("arm-summary").textContent = result.summary;
      await refresh();
    }

    async function applyTiltedStand() {
      const result = await postJson(tiltedStandApplyUrl, {
        pitch_deg: tiltedStandValue("pitch"),
        roll_deg: tiltedStandValue("roll"),
      });
      document.getElementById("tilted-stand-summary").textContent = result.summary;
      await refresh();
    }

    async function resetTiltedStand() {
      const result = await postJson(tiltedStandResetUrl, {});
      document.getElementById("tilted-stand-summary").textContent = result.summary;
      await refresh();
      setTiltedStandSlidersFromState(window.__tiltedStandState ?? { pitch_deg: 0, roll_deg: 0 }, true);
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

    async function applyManualAxisTorqueLimit(axis) {
      const result = await postJson(manualTorqueLimitUrl, {
        group_key: document.getElementById("manual-group").value,
        target: axis,
        torque_limit: manualAxisTorqueLimitValue(axis),
      });
      document.getElementById("manual-summary").textContent = result.summary;
      await refresh();
      setManualSlidersFromGroupValue(true);
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

    function setTiltedStandAxisFromInput(axis) {
      const input = document.getElementById(`tilted-stand-${axis}-input`);
      const slider = document.getElementById(`tilted-stand-${axis}-slider`);
      const value = Number(input.value);
      if (!Number.isFinite(value)) {
        updateTiltedStandReadout(axis);
        return;
      }
      const clamped = clampTiltedStandAxisValue(axis, value);
      slider.value = String(clamped);
      updateTiltedStandReadout(axis);
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
        const torqueInput = document.getElementById(`manual-${axis}-torque-limit`);
        torqueInput.addEventListener("change", () => {
          applyManualAxisTorqueLimit(axis).catch((error) => {
            document.getElementById("manual-summary").textContent = String(error);
          });
        });
        torqueInput.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyManualAxisTorqueLimit(axis).catch((error) => {
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

    function bindArmControls() {
      if (!window.__armActionButtonsBound) {
        window.__armActionButtonsBound = true;
        document.getElementById("arm-apply").addEventListener("click", () => applyArmPose().catch((error) => {
          document.getElementById("arm-summary").textContent = String(error);
        }));
        document.getElementById("arm-reset").addEventListener("click", () => resetArmPose().catch((error) => {
          document.getElementById("arm-summary").textContent = String(error);
        }));
        document.getElementById("arm-capture").addEventListener("click", () => captureArmZero().catch((error) => {
          document.getElementById("arm-summary").textContent = String(error);
        }));
        document.getElementById("arm-sync-current").addEventListener("click", () => syncArmTargetToCurrent().catch((error) => {
          document.getElementById("arm-summary").textContent = String(error);
        }));
      }
      if (window.__armControlsBound) return;
      window.__armControlsBound = true;
      for (const joint of window.__armJoints ?? []) {
        const slider = document.getElementById(`arm-${joint.key}-slider`);
        slider.addEventListener("input", () => {
          updateArmReadout(joint.key);
          scheduleLiveArmApply();
        });
        slider.addEventListener("change", () => {
          updateArmReadout(joint.key);
        });
        const input = document.getElementById(`arm-${joint.key}-input`);
        input.addEventListener("input", () => {
          setArmAxisFromInput(joint.key);
          scheduleLiveArmApply();
        });
        input.addEventListener("change", () => {
          setArmAxisFromInput(joint.key);
        });
        const jumpInput = document.getElementById(`arm-${joint.key}-jump`);
        const jumpButton = document.getElementById(`arm-${joint.key}-jump-apply`);
        jumpButton.addEventListener("click", () => {
          applyArmJointJump(joint.key).catch((error) => {
            document.getElementById("arm-summary").textContent = String(error);
          });
        });
        jumpInput.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyArmJointJump(joint.key).catch((error) => {
              document.getElementById("arm-summary").textContent = String(error);
            });
          }
        });
        const torqueInput = document.getElementById(`arm-${joint.key}-torque-limit`);
        torqueInput.addEventListener("change", () => {
          applyArmJointTorqueLimit(joint.key).catch((error) => {
            document.getElementById("arm-summary").textContent = String(error);
          });
        });
        torqueInput.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyArmJointTorqueLimit(joint.key).catch((error) => {
              document.getElementById("arm-summary").textContent = String(error);
            });
          }
        });
        updateArmReadout(joint.key);
      }
    }

    function bindTiltedStandControls() {
      if (window.__tiltedStandControlsBound) return;
      window.__tiltedStandControlsBound = true;
      for (const axis of TILTED_STAND_AXES) {
        const slider = document.getElementById(`tilted-stand-${axis}-slider`);
        slider.addEventListener("input", () => {
          updateTiltedStandReadout(axis);
          scheduleLiveTiltedStandApply();
        });
        slider.addEventListener("change", () => {
          updateTiltedStandReadout(axis);
        });
        const input = document.getElementById(`tilted-stand-${axis}-input`);
        input.addEventListener("input", () => {
          setTiltedStandAxisFromInput(axis);
          scheduleLiveTiltedStandApply();
        });
        input.addEventListener("change", () => {
          setTiltedStandAxisFromInput(axis);
        });
        updateTiltedStandReadout(axis);
      }
      document.getElementById("tilted-stand-apply").addEventListener("click", () => applyTiltedStand().catch((error) => {
        document.getElementById("tilted-stand-summary").textContent = String(error);
      }));
      document.getElementById("tilted-stand-reset").addEventListener("click", () => resetTiltedStand().catch((error) => {
        document.getElementById("tilted-stand-summary").textContent = String(error);
      }));
    }

    function updateManualPanel(manual) {
      window.__manualGroups = manual?.groups ?? [];
      window.__manualGroupValues = manual?.group_values ?? [];
      bindManualControls();
      ensureManualGroups(window.__manualGroups);
      syncManualSliderSpecs(manual?.joints ?? []);
      const enabled = Boolean(manual?.enabled);
      const ready = Boolean(manual?.ready);
      const becameEnabled = enabled && !manualPanelState.enabled;
      const becameReady = enabled && ready && !(manualPanelState.enabled && manualPanelState.ready);
      setManualSlidersFromGroupValue(
        !manualSlidersInitialized.value || becameEnabled || becameReady
      );
      manualPanelState.enabled = enabled;
      manualPanelState.ready = ready;

      document.getElementById("manual-summary").textContent = manual?.summary ?? "manual control unavailable";
      document.getElementById("manual-mode-state").textContent = enabled
        ? (ready ? "ready" : "waiting")
        : "disabled";
      document.getElementById("manual-mode-note").textContent = enabled
        ? (manual.base_pose_captured
            ? "Manual zero is captured for reset actions. Sliders show absolute semantic angles."
            : "Sliders show absolute semantic angles. Capture the current pose if you want reset-to-zero behavior.")
        : "Switch the motion mode to Manual to enable dashboard-based servo control.";

      setManualControlsEnabled(Boolean(enabled && manualGroupsReady));
    }

    function updateArmPanel(arm) {
      window.__armState = arm ?? null;
      window.__armJoints = arm?.joints ?? [];
      window.__armJointValues = arm?.joint_values ?? [];
      ensureArmSliderFields(window.__armJoints);
      bindArmControls();

      const configured = Boolean(arm);
      const enabled = Boolean(arm?.enabled);
      const ready = Boolean(arm?.ready);
      const becameConfigured = configured && !armPanelState.configured;
      const becameReady = enabled && ready && !(armPanelState.enabled && armPanelState.ready);
      const completedTorqueSync = Boolean(
        armTorqueSyncPending && arm?.summary?.startsWith("arm utility:")
      );
      setArmSlidersFromState(
        !armSlidersInitialized.value || becameConfigured || becameReady || completedTorqueSync
      );
      if (completedTorqueSync) {
        armTorqueSyncPending = false;
      }
      if (!configured || !enabled || arm?.summary?.startsWith("arm utility failed:")) {
        armTorqueSyncPending = false;
      }
      armPanelState.configured = configured;
      armPanelState.enabled = enabled;
      armPanelState.ready = ready;

      document.getElementById("arm-summary").textContent = arm?.summary ?? "arm unavailable";
      document.getElementById("arm-name").textContent = arm?.name ?? "No arm configured";
      document.getElementById("arm-bus-port").textContent = arm?.bus_port ?? "Load a profile with an arm store to enable this section.";
      document.getElementById("arm-mode-state").textContent = !configured
        ? "unconfigured"
        : enabled
          ? (ready ? "ready" : "waiting")
          : "disabled";
      document.getElementById("arm-mode-note").textContent = !configured
        ? "This profile does not currently load an arm configuration."
        : enabled
          ? (arm.base_pose_captured
              ? "Arm zero/home is captured. Sliders are relative to that pose."
              : "Waiting for a full arm feedback sweep before relative control becomes ready.")
          : "Switch the motion mode to Manual to enable dashboard-based arm control.";
      document.getElementById("arm-mount").textContent = armMountLabel(arm?.mount);
      document.getElementById("arm-servo-count").textContent = arm
        ? `${arm.online_servo_count} / ${arm.servos.length}`
        : "0 / 0";
      document.getElementById("arm-servo-count-note").textContent = arm
        ? `${arm.online_servo_count} / ${arm.servos.length} configured arm servos replying.`
        : "0 / 0 configured arm servos replying.";
      document.getElementById("arm-bus-note").textContent = arm?.last_poll_error
        ?? (arm ? "All configured arm servos replied on the last poll." : "Waiting for arm bus state.");
      document.getElementById("arm-layout-note").textContent = arm
        ? "Configured arm joints appear in the command order defined by the arm config."
        : "No arm servo layout is available for this profile.";
      document.getElementById("arm-servo-chain").innerHTML = renderArmServoChain(arm?.servos ?? []);

      setArmControlsEnabled(Boolean(enabled && ready && window.__armJoints.length));
    }

    function updateTiltedStandPanel(tiltedStand) {
      window.__tiltedStandState = tiltedStand ?? null;
      bindTiltedStandControls();
      const enabled = Boolean(tiltedStand?.enabled);
      const ready = Boolean(tiltedStand?.ready);
      const becameEnabled = enabled && !tiltedStandPanelState.enabled;
      const becameReady = enabled && ready && !(tiltedStandPanelState.enabled && tiltedStandPanelState.ready);
      setTiltedStandSlidersFromState(
        tiltedStand ?? { pitch_deg: 0, roll_deg: 0 },
        !tiltedStandSlidersInitialized.value || becameEnabled || becameReady
      );
      tiltedStandPanelState.enabled = enabled;
      tiltedStandPanelState.ready = ready;

      document.getElementById("tilted-stand-summary").textContent = tiltedStand?.summary ?? "tilted stand unavailable";
      document.getElementById("tilted-stand-mode-state").textContent = enabled
        ? (ready ? "ready" : "waiting")
        : "disabled";
      document.getElementById("tilted-stand-mode-note").textContent = enabled
        ? "Pitch and roll are applied around the stance captured when the mode armed."
        : "Switch the motion mode to Tilted Stand to enable pitch and roll body-tilt control.";
      document.getElementById("tilted-stand-limits-note").textContent =
        `Pitch limit ±${Number(tiltedStand?.pitch_limit_deg ?? 20).toFixed(1)}°, roll limit ±${Number(tiltedStand?.roll_limit_deg ?? 20).toFixed(1)}°.`;
      for (const axis of TILTED_STAND_AXES) {
        const limit = Number(tiltedStand?.[`${axis}_limit_deg`] ?? 20);
        document.getElementById(`tilted-stand-${axis}-slider`).min = String(-limit);
        document.getElementById(`tilted-stand-${axis}-slider`).max = String(limit);
        document.getElementById(`tilted-stand-${axis}-input`).min = String(-limit);
        document.getElementById(`tilted-stand-${axis}-input`).max = String(limit);
      }
      setTiltedStandControlsEnabled(enabled);
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

    function describeImuState(imu) {
      if (!imu) {
        return {
          summary: "IMU disabled",
          mode: "disabled",
          device: "No IMU section is configured for this profile.",
          sensorKind: "-",
          sensorNote: "-",
          attitude: "-",
          accelNote: "-",
          motion: "-",
          health: "-",
        };
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
      const health = [
        imu.telemetry?.temperature_c != null ? `temp ${fmt(imu.telemetry.temperature_c, 1)} °C` : null,
        `faults ${faults}`,
        imu.last_error ? compactError(imu.last_error) : null,
      ].filter(Boolean).join(" | ") || "No telemetry yet.";

      return {
        summary: imu.last_error ?? `${sensorKind} streaming`,
        mode: imu.enabled ? imu.mode : "disabled",
        device: imu.device ?? imu.description ?? "No device path",
        sensorKind,
        sensorNote,
        attitude,
        accelNote,
        motion,
        health,
      };
    }

    function updateRailImuPanel(view) {
      document.getElementById("rail-imu-summary").textContent = view.summary;
      document.getElementById("rail-imu-attitude").textContent = view.attitude;
      document.getElementById("rail-imu-attitude-note").textContent = view.accelNote;
      document.getElementById("rail-imu-motion").textContent = view.motion;
      document.getElementById("rail-imu-motion-note").textContent = view.health;
      document.getElementById("rail-imu-mode").textContent = view.mode;
      document.getElementById("rail-imu-device").textContent = view.device;
      document.getElementById("rail-imu-sensor-kind").textContent = view.sensorKind;
      document.getElementById("rail-imu-sensor-note").textContent = view.sensorNote;
    }

    function updateImuPanel(imu) {
      const view = describeImuState(imu);
      document.getElementById("imu-summary").textContent = view.summary;
      document.getElementById("imu-mode").textContent = view.mode;
      document.getElementById("imu-device").textContent = view.device;
      document.getElementById("imu-sensor-kind").textContent = view.sensorKind;
      document.getElementById("imu-sensor-note").textContent = view.sensorNote;
      document.getElementById("imu-attitude").textContent = view.attitude;
      document.getElementById("imu-accel-note").textContent = view.accelNote;
      document.getElementById("imu-motion").textContent = view.motion;
      document.getElementById("imu-health-note").textContent = view.health;
      updateRailImuPanel(view);
    }

    function robotScenePlaceholder(message) {
      const empty = document.getElementById("robot-scene-empty");
      const canvas = document.getElementById("robot-scene-canvas");
      empty.hidden = false;
      empty.textContent = message;
      canvas.hidden = true;
    }

    function robotSceneMixHex(fromHex, toHex, ratio) {
      const t = clamp(Number(ratio) || 0, 0, 1);
      const parse = (hex) => {
        const normalized = hex.replace("#", "");
        return [
          parseInt(normalized.slice(0, 2), 16),
          parseInt(normalized.slice(2, 4), 16),
          parseInt(normalized.slice(4, 6), 16),
        ];
      };
      const [r1, g1, b1] = parse(fromHex);
      const [r2, g2, b2] = parse(toHex);
      const toHexByte = (value) => Math.round(value).toString(16).padStart(2, "0");
      return `#${toHexByte(r1 + (r2 - r1) * t)}${toHexByte(g1 + (g2 - g1) * t)}${toHexByte(b1 + (b2 - b1) * t)}`;
    }

    function robotSceneJointTelemetry(servo, jointIndex) {
      const telemetry = servo?.telemetry;
      const loadPct = Math.abs(Number(telemetry?.present_load_pct));
      const currentMa = Number(telemetry?.present_current_ma);
      return {
        jointIndex,
        jointKey: JOINT_LABEL[jointIndex] ?? `joint ${jointIndex}`,
        online: Boolean(servo?.online),
        loadPct: servo?.online && Number.isFinite(loadPct) ? loadPct : 0,
        currentMa: servo?.online && Number.isFinite(currentMa) ? currentMa : 0,
        faultCount: servo?.online ? (telemetry?.faults?.length ?? 0) : 0,
      };
    }

    function robotSceneEmptyLegFeedback() {
      return {
        onlineJointCount: 0,
        avgLoadPct: 0,
        maxLoadPct: 0,
        avgCurrentMa: 0,
        maxCurrentMa: 0,
        faultCount: 0,
        joints: [1, 2, 3].map((jointIndex) => robotSceneJointTelemetry(null, jointIndex)),
        peakJointKey: null,
        primaryFaultJointKey: null,
      };
    }

    function robotSceneLegTelemetryByKey(servos) {
      const grouped = groupServosByLeg(servos ?? []);
      return Object.fromEntries(LEG_ORDER.map((legKey) => {
        const legServos = grouped[legKey] ?? [];
        const byJoint = servoByJoint(legServos);
        const jointFeedback = [1, 2, 3].map((jointIndex) =>
          robotSceneJointTelemetry(byJoint[jointIndex], jointIndex)
        );
        const onlineJoints = jointFeedback.filter((joint) => joint.online);
        const loads = onlineJoints.map((joint) => joint.loadPct);
        const currents = onlineJoints.map((joint) => joint.currentMa);
        const faultCount = onlineJoints.reduce((count, joint) => count + joint.faultCount, 0);
        const avgLoadPct = loads.length
          ? loads.reduce((sum, value) => sum + value, 0) / loads.length
          : 0;
        const maxLoadPct = loads.length ? Math.max(...loads) : 0;
        const avgCurrentMa = currents.length
          ? currents.reduce((sum, value) => sum + value, 0) / currents.length
          : 0;
        const maxCurrentMa = currents.length ? Math.max(...currents) : 0;
        const peakJoint = [...onlineJoints]
          .sort((left, right) => right.loadPct - left.loadPct)[0] ?? null;
        const primaryFaultJoint = [...onlineJoints]
          .filter((joint) => joint.faultCount > 0)
          .sort((left, right) => (
            right.faultCount - left.faultCount
              || right.loadPct - left.loadPct
              || left.jointIndex - right.jointIndex
          ))[0] ?? null;
        return [legKey, {
          onlineJointCount: onlineJoints.length,
          avgLoadPct,
          maxLoadPct,
          avgCurrentMa,
          maxCurrentMa,
          faultCount,
          joints: jointFeedback,
          peakJointKey: peakJoint?.jointKey ?? null,
          primaryFaultJointKey: primaryFaultJoint?.jointKey ?? null,
        }];
      }));
    }

    function robotSceneJointColor(feedback) {
      if (!feedback?.online) {
        return {
          segment: "#5a6775",
          joint: "#758292",
          foot: "#5a6775",
          emissive: "#3c4651",
          emphasis: 0.0,
        };
      }
      if (feedback.faultCount > 0) {
        return {
          segment: "#ff6f61",
          joint: "#ffd0cb",
          foot: "#ff6f61",
          emissive: "#ff897c",
          emphasis: 1.0,
        };
      }

      const loadRatio = clamp(feedback.loadPct / 100, 0, 1);
      let segment = "#77e4b5";
      let foot = "#77e4b5";
      if (loadRatio > 0.55) {
        const t = (loadRatio - 0.55) / 0.45;
        segment = robotSceneMixHex("#ffb15b", "#ff6f61", t);
        foot = robotSceneMixHex("#ffc26b", "#ff6f61", t);
      } else if (loadRatio > 0.18) {
        const t = (loadRatio - 0.18) / 0.37;
        segment = robotSceneMixHex("#77e4b5", "#ffb15b", t);
        foot = robotSceneMixHex("#95efc4", "#ffc26b", t);
      }

      return {
        segment,
        joint: robotSceneMixHex("#eef3f7", segment, 0.32),
        foot,
        emissive: robotSceneMixHex("#6be6d2", foot, 0.58),
        emphasis: loadRatio,
      };
    }

    function robotSceneJointNodeVisual(primarySegment, secondarySegment = null) {
      const from = primarySegment ?? robotSceneJointColor(null);
      const to = secondarySegment ?? from;
      return {
        color: robotSceneMixHex(from.joint, to.joint, secondarySegment ? 0.5 : 0),
        emissive: robotSceneMixHex(from.emissive, to.emissive, secondarySegment ? 0.5 : 0),
        emphasis: Math.max(from.emphasis, to.emphasis),
      };
    }

    function robotSceneLegColor(feedback) {
      const jointVisualsByKey = Object.fromEntries(
        (feedback?.joints ?? []).map((joint) => [joint.jointKey, robotSceneJointColor(joint)])
      );
      const coxaVisual = jointVisualsByKey.coxa ?? robotSceneJointColor(null);
      const femurVisual = jointVisualsByKey.femur ?? robotSceneJointColor(null);
      const tibiaVisual = jointVisualsByKey.tibia ?? robotSceneJointColor(null);

      return {
        segments: [coxaVisual, femurVisual, tibiaVisual],
        joints: [
          robotSceneJointNodeVisual(coxaVisual),
          robotSceneJointNodeVisual(coxaVisual, femurVisual),
          robotSceneJointNodeVisual(femurVisual, tibiaVisual),
        ],
        foot: tibiaVisual.foot,
        footEmissive: tibiaVisual.emissive,
        emphasis: Math.max(
          coxaVisual.emphasis,
          femurVisual.emphasis,
          tibiaVisual.emphasis,
        ),
      };
    }

    function robotSceneFeedbackClass(feedback) {
      if (!feedback || feedback.onlineJointCount === 0) return "";
      if (feedback.faultCount > 0 || feedback.maxLoadPct >= 55) return "high";
      if (feedback.maxLoadPct >= 22) return "medium";
      return "low";
    }

    function renderRobotSceneFeedback(servos) {
      const feedbackByKey = robotSceneLegTelemetryByKey(servos);
      return LEG_ORDER.map((legKey) => {
        const feedback = feedbackByKey[legKey];
        const meta = LEG_META[legKey];
        const loadText = feedback?.onlineJointCount
          ? feedback.primaryFaultJointKey
            ? `${feedback.primaryFaultJointKey} fault`
            : feedback.peakJointKey
              ? `${feedback.peakJointKey} ${fmt(feedback.maxLoadPct, 0)}%`
              : `${fmt(feedback.maxLoadPct, 0)}% load`
          : "offline";
        const currentText = feedback?.onlineJointCount
          ? `${Math.round(feedback.maxCurrentMa || 0)} mA`
          : "no current";
        const healthText = feedback?.faultCount
          ? `${feedback.faultCount} fault${feedback.faultCount === 1 ? "" : "s"} on ${feedback.primaryFaultJointKey ?? "leg"}`
          : `${feedback?.onlineJointCount ?? 0}/3 joints`;
        return `
          <div class="robot-scene-feedback-chip ${robotSceneFeedbackClass(feedback)}">
            <div class="robot-scene-feedback-top">
              <span class="robot-scene-feedback-name">${meta.label}</span>
              <span class="robot-scene-feedback-load">${loadText}</span>
            </div>
            <div class="robot-scene-feedback-meta">${currentText} · ${healthText}</div>
          </div>
        `;
      }).join("");
    }

    function robotSceneVector(point) {
      return new THREE_NS.Vector3(
        Number(point?.x ?? 0),
        Number(point?.y ?? 0),
        Number(point?.z ?? 0),
      );
    }

    function robotSceneAxisVector(forward, left, up) {
      return new THREE_NS.Vector3(forward, left, up);
    }

    function disposeRobotSceneObject(object) {
      object.traverse((child) => {
        child.geometry?.dispose?.();
        if (Array.isArray(child.material)) {
          child.material.forEach((material) => material?.dispose?.());
        } else {
          child.material?.dispose?.();
        }
      });
    }

    function resizeRobotSceneRuntime(runtime) {
      const width = Math.max(Math.round(runtime.host.clientWidth), 2);
      const height = Math.max(Math.round(runtime.host.clientHeight), 2);
      if (width === runtime.width && height === runtime.height) return;
      runtime.width = width;
      runtime.height = height;
      runtime.camera.aspect = width / height;
      runtime.camera.updateProjectionMatrix();
      runtime.renderer.setSize(width, height, false);
    }

    function resetRobotSceneCamera(runtime) {
      runtime.camera.position.copy(runtime.defaultCameraPosition);
      runtime.controls.target.copy(runtime.defaultTarget);
      runtime.controls.update();
    }

    function primeRobotSceneCamera(runtime, root) {
      const bounds = new THREE_NS.Box3().setFromObject(root);
      if (!Number.isFinite(bounds.min.x) || !Number.isFinite(bounds.max.x)) {
        return;
      }
      const size = bounds.getSize(new THREE_NS.Vector3());
      const center = bounds.getCenter(new THREE_NS.Vector3());
      const distance = Math.max(size.length() * 1.1, 26);
      runtime.defaultTarget.copy(center);
      runtime.defaultCameraPosition.copy(
        center.clone().add(new THREE_NS.Vector3(-1.36, -1.02, 0.84).normalize().multiplyScalar(distance))
      );
      runtime.cameraPrimed = true;
      resetRobotSceneCamera(runtime);
    }

    function buildRobotSceneSegment(start, end, radius, color, emissive = color, emissiveIntensity = 0.0) {
      const direction = end.clone().sub(start);
      const length = direction.length();
      if (length < 0.001) {
        return null;
      }
      const mesh = new THREE_NS.Mesh(
        new THREE_NS.CylinderGeometry(radius, radius, length, 12, 1, false),
        new THREE_NS.MeshStandardMaterial({
          color,
          emissive,
          emissiveIntensity,
          roughness: 0.42,
          metalness: 0.08,
        }),
      );
      mesh.position.copy(start.clone().add(end).multiplyScalar(0.5));
      mesh.quaternion.setFromUnitVectors(new THREE_NS.Vector3(0, 1, 0), direction.normalize());
      return mesh;
    }

    function buildRobotSceneRoot(bodyScene, imu, servos) {
      const root = new THREE_NS.Group();
      const pitchGroup = new THREE_NS.Group();
      const rollGroup = new THREE_NS.Group();
      const feedbackByKey = robotSceneLegTelemetryByKey(servos);
      root.add(pitchGroup);
      pitchGroup.add(rollGroup);
      // Keep the scene in direct ROS coordinates: +x forward, +y left, +z up.
      // Positive pitch raises the nose, and positive roll raises the left side.
      pitchGroup.rotation.y = -Number(imu?.pitch_deg ?? 0) * Math.PI / 180;
      rollGroup.rotation.x = Number(imu?.roll_deg ?? 0) * Math.PI / 180;

      const outline = bodyScene.body_outline ?? [];
      if (outline.length >= 3) {
        const outlinePoints = outline.map((point) => robotSceneVector(point));
        const bodyShape = new THREE_NS.Shape(
          outlinePoints.map((point) => new THREE_NS.Vector2(point.x, point.y))
        );
        const thickness = 1.0;
        const plateGeometry = new THREE_NS.ExtrudeGeometry(bodyShape, {
          depth: thickness,
          bevelEnabled: false,
        });
        plateGeometry.translate(0, 0, -thickness / 2);
        const bodyPlate = new THREE_NS.Mesh(
          plateGeometry,
          new THREE_NS.MeshStandardMaterial({
            color: "#1f3342",
            transparent: true,
            opacity: 0.88,
            roughness: 0.62,
            metalness: 0.06,
            side: THREE_NS.DoubleSide,
          }),
        );
        rollGroup.add(bodyPlate);

        const outlineLoop = new THREE_NS.LineLoop(
          new THREE_NS.BufferGeometry().setFromPoints(outlinePoints),
          new THREE_NS.LineBasicMaterial({
            color: "#8fcfff",
            transparent: true,
            opacity: 0.82,
          }),
        );
        outlineLoop.position.z = thickness * 0.12;
        rollGroup.add(outlineLoop);
      }

      for (const leg of bodyScene.legs ?? []) {
        if (!leg.pose) continue;
        const feedback = feedbackByKey[leg.leg_key] ?? robotSceneEmptyLegFeedback();
        const color = robotSceneLegColor(feedback);
        const points = [
          robotSceneVector(leg.pose.anchor),
          robotSceneVector(leg.pose.coxa_end),
          robotSceneVector(leg.pose.femur_end),
          robotSceneVector(leg.pose.tibia_end),
        ];
        const segmentRadii = [0.24, 0.22, 0.20].map((radius, index) => {
          const segmentVisual = color.segments[index];
          return radius * (1.0 + (segmentVisual?.emphasis ?? 0) * 0.38);
        });
        for (let index = 0; index < points.length - 1; index += 1) {
          const segmentVisual = color.segments[index];
          const segment = buildRobotSceneSegment(
            points[index],
            points[index + 1],
            segmentRadii[index] ?? 0.20,
            segmentVisual?.segment ?? "#5a6775",
            segmentVisual?.emissive ?? "#3c4651",
            0.05 + (segmentVisual?.emphasis ?? 0) * 0.22,
          );
          if (segment) {
            rollGroup.add(segment);
          }
        }
        for (const [index, point] of points.slice(0, -1).entries()) {
          const jointVisual = color.joints[index] ?? robotSceneJointNodeVisual(null);
          const joint = new THREE_NS.Mesh(
            new THREE_NS.SphereGeometry(0.30, 14, 12),
            new THREE_NS.MeshStandardMaterial({
              color: jointVisual.color,
              emissive: jointVisual.emissive,
              emissiveIntensity: 0.02 + jointVisual.emphasis * 0.16,
              roughness: 0.24,
              metalness: 0.04,
            }),
          );
          joint.position.copy(point);
          rollGroup.add(joint);
        }
        const foot = new THREE_NS.Mesh(
          new THREE_NS.SphereGeometry(0.36 + color.emphasis * 0.12, 16, 14),
          new THREE_NS.MeshStandardMaterial({
            color: color.foot,
            emissive: color.footEmissive,
            emissiveIntensity: 0.10 + color.emphasis * 0.34,
            roughness: 0.18,
            metalness: 0.02,
          }),
        );
        foot.position.copy(points[points.length - 1]);
        rollGroup.add(foot);
      }

      const imuBase = robotSceneVector(bodyScene.imu_position_cm ?? { x: 0, y: 0, z: 0 });
      const imuStemTop = imuBase.clone().add(robotSceneAxisVector(0, 0, 1.8));
      const imuColor = bodyScene.imu_mount_configured ? "#ffd36f" : "#8ecae6";
      const imuStem = buildRobotSceneSegment(imuBase, imuStemTop, 0.08, imuColor);
      if (imuStem) {
        rollGroup.add(imuStem);
      }
      const imuBoard = new THREE_NS.Mesh(
        new THREE_NS.BoxGeometry(1.9, 0.36, 1.2),
        new THREE_NS.MeshStandardMaterial({
          color: imuColor,
          roughness: 0.34,
          metalness: 0.06,
        }),
      );
      imuBoard.position.copy(imuBase.clone().add(robotSceneAxisVector(0, 0, 0.34)));
      rollGroup.add(imuBoard);
      const imuTip = new THREE_NS.Mesh(
        new THREE_NS.ConeGeometry(0.26, 0.72, 14),
        new THREE_NS.MeshStandardMaterial({
          color: imuColor,
          roughness: 0.30,
          metalness: 0.04,
        }),
      );
      imuTip.position.copy(imuStemTop.clone().add(robotSceneAxisVector(0, 0, 0.34)));
      rollGroup.add(imuTip);

      const axisOrigin = robotSceneAxisVector(0, 0, 1.1);
      const axisSpecs = [
        { direction: robotSceneAxisVector(1, 0, 0).normalize(), color: "#ff9254" },
        { direction: robotSceneAxisVector(0, 1, 0).normalize(), color: "#7dc8ff" },
        { direction: robotSceneAxisVector(0, 0, 1).normalize(), color: "#9cf0a8" },
      ];
      for (const axis of axisSpecs) {
        rollGroup.add(new THREE_NS.ArrowHelper(axis.direction, axisOrigin, 4.7, axis.color, 0.78, 0.30));
      }

      return root;
    }

    function ensureRobotSceneRuntime() {
      if (robotSceneRuntime) {
        return robotSceneRuntime;
      }
      if (!THREE_NS || !OrbitControlsCtor) {
        return null;
      }

      const host = document.getElementById("robot-scene-canvas");
      const scene = new THREE_NS.Scene();
      scene.fog = new THREE_NS.Fog(0x081018, 36, 128);

      const camera = new THREE_NS.PerspectiveCamera(34, 1, 0.1, 320);
      camera.up.set(0, 0, 1);
      const renderer = new THREE_NS.WebGLRenderer({
        antialias: true,
        alpha: true,
        powerPreference: "high-performance",
      });
      renderer.setPixelRatio(Math.min(window.devicePixelRatio ?? 1, 2));
      renderer.outputColorSpace = THREE_NS.SRGBColorSpace;
      renderer.toneMapping = THREE_NS.ACESFilmicToneMapping;
      renderer.toneMappingExposure = 1.0;
      renderer.domElement.setAttribute("aria-hidden", "true");
      host.replaceChildren(renderer.domElement);

      const controls = new OrbitControlsCtor(camera, renderer.domElement);
      controls.enableDamping = true;
      controls.dampingFactor = 0.06;
      controls.minDistance = 12;
      controls.maxDistance = 120;
      controls.target.set(0, 0, 0);

      const ambient = new THREE_NS.AmbientLight(0xd9e8ff, 0.76);
      const keyLight = new THREE_NS.DirectionalLight(0xfff2d6, 1.2);
      keyLight.position.set(16, 12, 20);
      const rimLight = new THREE_NS.DirectionalLight(0x86cfff, 0.52);
      rimLight.position.set(-14, -18, 8);
      const grid = new THREE_NS.GridHelper(60, 24, 0x344758, 0x16202a);
      grid.material.transparent = true;
      grid.material.opacity = 0.34;
      grid.rotation.x = Math.PI / 2;
      scene.add(ambient, keyLight, rimLight, grid);

      const runtime = {
        host,
        scene,
        camera,
        renderer,
        controls,
        grid,
        width: 0,
        height: 0,
        root: null,
        cameraPrimed: false,
        defaultTarget: new THREE_NS.Vector3(0, 0, 0),
        defaultCameraPosition: new THREE_NS.Vector3(-24, -18, 18),
      };
      resizeRobotSceneRuntime(runtime);
      resetRobotSceneCamera(runtime);

      const tick = () => {
        runtime.controls.update();
        runtime.renderer.render(runtime.scene, runtime.camera);
        window.requestAnimationFrame(tick);
      };
      window.requestAnimationFrame(tick);

      if (typeof ResizeObserver === "function") {
        runtime.resizeObserver = new ResizeObserver(() => resizeRobotSceneRuntime(runtime));
        runtime.resizeObserver.observe(host);
      } else {
        window.addEventListener("resize", () => resizeRobotSceneRuntime(runtime));
      }

      renderer.domElement.addEventListener("dblclick", () => resetRobotSceneCamera(runtime));
      robotSceneRuntime = runtime;
      return runtime;
    }

    function updateRobotScene(bodyScene, imu, servos = []) {
      const liveLegs = bodyScene?.legs?.filter((leg) => leg.online_joint_count === 3).length ?? 0;
      const totalLegs = bodyScene?.legs?.length ?? 0;
      const feedbackByKey = robotSceneLegTelemetryByKey(servos);
      const strongestLeg = LEG_ORDER
        .map((legKey) => ({ legKey, feedback: feedbackByKey[legKey] }))
        .filter((entry) => entry.feedback?.onlineJointCount)
        .sort((left, right) => (right.feedback.maxLoadPct ?? 0) - (left.feedback.maxLoadPct ?? 0))[0];
      document.getElementById("robot-scene-summary").textContent = bodyScene
        ? strongestLeg
          ? `${liveLegs}/${totalLegs} live · max load ${LEG_META[strongestLeg.legKey].label} ${strongestLeg.feedback.peakJointKey ?? "leg"} ${fmt(strongestLeg.feedback.maxLoadPct, 0)}%`
          : `${liveLegs}/${totalLegs} legs live`
        : "ROS body frame";
      if (robotSceneModuleError) {
        robotScenePlaceholder("3D renderer unavailable. The browser could not load three.js or OrbitControls.");
        document.getElementById("robot-scene-note").textContent =
          "The dashboard stayed online, but the WebGL body view could not load its three.js modules. If you want this fully offline, I can vendor the renderer assets next.";
        document.getElementById("robot-scene-feedback").innerHTML = '<div class="stat-note">3D feedback overlay unavailable while the renderer modules are missing.</div>';
        return;
      }

      if (!bodyScene?.legs?.length) {
        robotScenePlaceholder("Waiting for live body geometry.");
        document.getElementById("robot-scene-feedback").innerHTML = '<div class="stat-note">Waiting for live servo load feedback.</div>';
        return;
      }

      const runtime = ensureRobotSceneRuntime();
      if (!runtime) {
        robotScenePlaceholder("WebGL renderer not ready yet.");
        return;
      }

      const canvas = document.getElementById("robot-scene-canvas");
      const empty = document.getElementById("robot-scene-empty");
      empty.hidden = true;
      canvas.hidden = false;
      resizeRobotSceneRuntime(runtime);

      if (runtime.root) {
        runtime.scene.remove(runtime.root);
        disposeRobotSceneObject(runtime.root);
      }

      const root = buildRobotSceneRoot(bodyScene, imu, servos);
      runtime.root = root;
      runtime.scene.add(root);
      const bounds = new THREE_NS.Box3().setFromObject(root);
      if (Number.isFinite(bounds.min.z)) {
        runtime.grid.position.z = bounds.min.z - 0.06;
      }
      if (!runtime.cameraPrimed) {
        primeRobotSceneCamera(runtime, root);
      }

      document.getElementById("robot-scene-note").textContent = bodyScene.imu_mount_configured
        ? "The WebGL body view follows the ROS body frame and uses the configured IMU mount offset. Leg segments and glow now reflect live servo load and resistance."
        : "The WebGL body view follows the ROS body frame. Leg segments and glow now reflect live servo load and resistance; the IMU marker stays at the body origin until an IMU mount offset is configured.";
      document.getElementById("robot-scene-feedback").innerHTML = renderRobotSceneFeedback(servos);
    }

    function renderServoNode(servo, labelOverride = null) {
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
      const jointLabel = labelOverride ?? servo.label ?? JOINT_LABEL[jointIndex] ?? "joint";
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
          ${sorted.map((servo) => renderServoNode(servo, JOINT_LABEL[jointIndexForServo(servo)] ?? "joint")).join("")}
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

    function renderArmServoChain(servos) {
      if (!servos.length) {
        return '<div class="stat-note">No arm servos are configured for this profile.</div>';
      }
      return servos
        .map((servo) => renderServoNode(servo, servo.label ?? `servo ${servo.servo_id}`))
        .join("");
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

    function previewPlaceholder({
      title,
      countText,
      shellClass,
      width = 220,
      height = 116,
    }) {
      const rectX = 16;
      const rectY = 16;
      const rectWidth = width - rectX * 2;
      const rectHeight = height - rectY * 2;
      const labelX = width / 2;
      const labelY = height / 2 - 2;
      const noteY = height / 2 + 14;
      const titleFontSize = width < 180 ? 10 : 12;
      const noteFontSize = width < 180 ? 9 : 11;
      return `
        <div class="leg-preview-shell ${shellClass}">
          <div class="leg-preview-top">
            <strong>${title}</strong>
            <span>${countText}</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 ${width} ${height}" aria-label="${title} pose unavailable">
            <rect x="${rectX}" y="${rectY}" width="${rectWidth}" height="${rectHeight}" rx="16" fill="rgba(255,255,255,0.03)" stroke="rgba(255,255,255,0.08)" />
            <text x="${labelX}" y="${labelY}" text-anchor="middle" fill="rgba(238,243,247,0.78)" font-size="${titleFontSize}">preview unavailable</text>
            <text x="${labelX}" y="${noteY}" text-anchor="middle" fill="rgba(148,164,182,0.92)" font-size="${noteFontSize}">need fresh semantic telemetry</text>
          </svg>
        </div>
      `;
    }

    function renderLegBirdPreview(legKey, servos, options = {}) {
      const meta = LEG_META[legKey];
      const onlineCount = servos.filter((servo) => servo.online).length;
      const width = options.width ?? 220;
      const height = options.height ?? 116;
      const title = options.title ?? "Top view";
      const countText = options.countText ?? `${onlineCount}/3 online`;
      const shellClass = options.shellClass ?? "center";
      const rawPose = currentLegPreview(legKey)?.top_view;
      if (!rawPose) {
        return previewPlaceholder({ title, countText, shellClass, width, height });
      }
      const pose = fitPreviewPose(rawPose, width, height);
      const scale = Math.min(width / 220, height / 116);
      const stroke = onlineCount === 3 ? '#ff9254' : (onlineCount > 0 ? '#ffc26b' : '#5a6775');
      const fill = onlineCount === 3 ? "rgba(255,146,84,0.12)" : "rgba(255,255,255,0.04)";
      const inwardDx = Math.sign(pose.anchor.x - pose.coxaEnd.x) || (meta.side === "left" ? 1 : -1);
      const bodyGuideStart = { x: pose.anchor.x + inwardDx * 20 * scale, y: pose.anchor.y };
      const bodyGuideEnd = { x: pose.anchor.x + inwardDx * 4 * scale, y: pose.anchor.y };

      return `
        <div class="leg-preview-shell ${shellClass}">
          <div class="leg-preview-top">
            <strong>${title}</strong>
            <span>${countText}</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 ${width} ${height}" aria-label="${meta.label} top-view live pose">
            <path d='M ${bodyGuideStart.x.toFixed(1)} ${bodyGuideStart.y.toFixed(1)} L ${bodyGuideEnd.x.toFixed(1)} ${bodyGuideEnd.y.toFixed(1)}'
              fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='${(10 * scale).toFixed(2)}' stroke-linecap='round' />
            <circle cx='${pose.anchor.x.toFixed(1)}' cy='${pose.anchor.y.toFixed(1)}' r='${(9 * scale).toFixed(2)}' fill='${fill}' stroke='rgba(255,255,255,0.10)' />
            <path d='M ${pose.anchor.x.toFixed(1)} ${pose.anchor.y.toFixed(1)} L ${pose.coxaEnd.x.toFixed(1)} ${pose.coxaEnd.y.toFixed(1)} L ${pose.femurEnd.x.toFixed(1)} ${pose.femurEnd.y.toFixed(1)} L ${pose.tibiaEnd.x.toFixed(1)} ${pose.tibiaEnd.y.toFixed(1)}'
              fill='none' stroke='${stroke}' stroke-width='${(9 * scale).toFixed(2)}' stroke-linecap='round' stroke-linejoin='round' />
            <circle cx='${pose.anchor.x.toFixed(1)}' cy='${pose.anchor.y.toFixed(1)}' r='${(6.5 * scale).toFixed(2)}' fill='#eef3f7' />
            <circle cx='${pose.coxaEnd.x.toFixed(1)}' cy='${pose.coxaEnd.y.toFixed(1)}' r='${(5.5 * scale).toFixed(2)}' fill='#d9e2ec' />
            <circle cx='${pose.femurEnd.x.toFixed(1)}' cy='${pose.femurEnd.y.toFixed(1)}' r='${(5.2 * scale).toFixed(2)}' fill='#c8d3de' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='${(5.2 * scale).toFixed(2)}' fill='${stroke}' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='${(9 * scale).toFixed(2)}' fill='none' stroke='${stroke}' stroke-width='${(1.6 * scale).toFixed(2)}' opacity='0.5' />
          </svg>
        </div>
      `;
    }

    function renderLegSidePreview(legKey, servos, options = {}) {
      const meta = LEG_META[legKey];
      const onlineCount = servos.filter((servo) => servo.online).length;
      const width = options.width ?? 220;
      const height = options.height ?? 116;
      const title = options.title ?? "Side view";
      const countText = options.countText ?? `${onlineCount}/3`;
      const shellClass = options.shellClass ?? "outer";
      const rawPose = currentLegPreview(legKey)?.side_view;
      if (!rawPose) {
        return previewPlaceholder({ title, countText, shellClass, width, height });
      }
      const pose = fitPreviewPose(rawPose, width, height);
      const scale = Math.min(width / 220, height / 116);
      const stroke = onlineCount === 3 ? '#7dc8ff' : (onlineCount > 0 ? '#b8dfff' : '#5a6775');
      const inwardDx = Math.sign(pose.anchor.x - pose.coxaEnd.x) || (meta.side === "left" ? 1 : -1);
      const bodyGuideStart = { x: pose.anchor.x + inwardDx * 20 * scale, y: pose.anchor.y };
      const bodyGuideEnd = { x: pose.anchor.x + inwardDx * 4 * scale, y: pose.anchor.y };

      return `
        <div class="leg-preview-shell ${shellClass}">
          <div class="leg-preview-top">
            <strong>${title}</strong>
            <span>${countText}</span>
          </div>
          <svg class="leg-preview-svg" viewBox="0 0 ${width} ${height}" aria-label="${meta.label} side-view live pose">
            <path d='M ${bodyGuideStart.x.toFixed(1)} ${bodyGuideStart.y.toFixed(1)} L ${bodyGuideEnd.x.toFixed(1)} ${bodyGuideEnd.y.toFixed(1)}'
              fill='none' stroke='rgba(255,255,255,0.14)' stroke-width='${(10 * scale).toFixed(2)}' stroke-linecap='round' />
            <path d='M ${pose.anchor.x.toFixed(1)} ${pose.anchor.y.toFixed(1)} L ${pose.coxaEnd.x.toFixed(1)} ${pose.coxaEnd.y.toFixed(1)} L ${pose.femurEnd.x.toFixed(1)} ${pose.femurEnd.y.toFixed(1)} L ${pose.tibiaEnd.x.toFixed(1)} ${pose.tibiaEnd.y.toFixed(1)}'
              fill='none' stroke='${stroke}' stroke-width='${(9 * scale).toFixed(2)}' stroke-linecap='round' stroke-linejoin='round' />
            <circle cx='${pose.anchor.x.toFixed(1)}' cy='${pose.anchor.y.toFixed(1)}' r='${(6.5 * scale).toFixed(2)}' fill='#eef3f7' />
            <circle cx='${pose.coxaEnd.x.toFixed(1)}' cy='${pose.coxaEnd.y.toFixed(1)}' r='${(5.5 * scale).toFixed(2)}' fill='#d9e2ec' />
            <circle cx='${pose.femurEnd.x.toFixed(1)}' cy='${pose.femurEnd.y.toFixed(1)}' r='${(5.2 * scale).toFixed(2)}' fill='#c8d3de' />
            <circle cx='${pose.tibiaEnd.x.toFixed(1)}' cy='${pose.tibiaEnd.y.toFixed(1)}' r='${(5.2 * scale).toFixed(2)}' fill='${stroke}' />
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
          ${meta.side === "left" ? `${outerPreview}${centerPreview}` : `${centerPreview}${outerPreview}`}
        </div>
      `;
    }

    function renderCompactLegPreviewCard(legKey, servos) {
      const meta = LEG_META[legKey];
      const onlineCount = servos.filter((servo) => servo.online).length;
      const topPreview = renderLegBirdPreview(legKey, servos, {
        width: 160,
        height: 88,
        title: "Top",
        countText: onlineCount === 3 ? "live" : `${onlineCount}/3`,
        shellClass: "compact center",
      });
      const sidePreview = renderLegSidePreview(legKey, servos, {
        width: 160,
        height: 88,
        title: "Side",
        countText: onlineCount === 3 ? "live" : `${onlineCount}/3`,
        shellClass: "compact outer",
      });

      return `
        <article class="rail-leg-card ${meta.railClass} ${onlineCount === 3 ? "live" : ""}">
          <div class="rail-leg-top">
            <div class="rail-leg-name">${meta.label}</div>
            <div class="rail-leg-count">${onlineCount}/3</div>
          </div>
          <div class="rail-leg-previews">
            ${meta.side === "left" ? `${sidePreview}${topPreview}` : `${topPreview}${sidePreview}`}
          </div>
        </article>
      `;
    }

    function renderCompactLegRail(servos) {
      const grouped = groupServosByLeg(servos);
      return `
        <div class="rail-leg-body" aria-hidden="true">
          <div class="rail-leg-axis">Front</div>
          <div class="rail-leg-core">
            <div class="rail-leg-side left">Left</div>
            <div class="rail-leg-side right">Right</div>
          </div>
          <div class="rail-leg-axis">Rear</div>
        </div>
        ${LEG_ORDER.map((legKey) => renderCompactLegPreviewCard(legKey, grouped[legKey])).join("")}
      `;
    }

    function setRailVisualTab(tabKey) {
      const activeTab = tabKey === "body" ? "body" : "legs";
      railVisualTabState.active = activeTab;
      const legsButton = document.getElementById("rail-visual-tab-legs");
      const bodyButton = document.getElementById("rail-visual-tab-body");
      const legsPane = document.getElementById("rail-visual-pane-legs");
      const bodyPane = document.getElementById("rail-visual-pane-body");
      const showingBody = activeTab === "body";

      legsButton.classList.toggle("active", !showingBody);
      bodyButton.classList.toggle("active", showingBody);
      legsButton.setAttribute("aria-selected", String(!showingBody));
      bodyButton.setAttribute("aria-selected", String(showingBody));
      legsPane.hidden = showingBody;
      bodyPane.hidden = !showingBody;

      if (showingBody && window.__latestState) {
        updateRobotScene(
          window.__latestState.body_scene,
          window.__latestState.imu,
          window.__latestState.servos ?? [],
        );
      }
    }

    function bindRailVisualTabs() {
      if (window.__railVisualTabsBound) return;
      window.__railVisualTabsBound = true;
      document.getElementById("rail-visual-tab-legs").addEventListener("click", () => {
        setRailVisualTab("legs");
      });
      document.getElementById("rail-visual-tab-body").addEventListener("click", () => {
        setRailVisualTab("body");
      });
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
        const armServos = state.arm?.servos ?? [];
        const totalOnlineServoCount = Number(state.online_servo_count ?? 0) + Number(state.arm?.online_servo_count ?? 0);
        const totalConfiguredServoCount = (state.servos?.length ?? 0) + armServos.length;

        document.getElementById("deployment-profile").textContent = state.deployment_profile;
        document.getElementById("compute-target").textContent = state.compute_target;
        document.getElementById("servo-count").textContent = `${totalOnlineServoCount} / ${totalConfiguredServoCount}`;
        document.getElementById("serial-port").textContent = state.serial_port;
        document.getElementById("serial-note").textContent = state.last_poll_error ?? "All configured servos replied on the last poll.";
        document.getElementById("camera-backend").textContent = state.camera_backend;
        document.getElementById("camera-note").textContent = state.camera_device ?? state.camera_pipeline;
        document.getElementById("camera-meta").textContent = state.camera_backend;
        document.getElementById("camera-rail-note").textContent = state.camera_device ?? state.camera_pipeline;
        document.getElementById("motion-mode").textContent = state.motion_mode ?? "-";
        document.getElementById("motion-summary").textContent = state.motion_summary ?? "-";
        document.getElementById("safety-status").textContent = state.motion_fault ? "tripped" : (state.safety_status ?? "ok");
        document.getElementById("motion-fault").textContent = state.motion_fault ?? "No safety trips latched.";
        document.getElementById("updated-at").textContent = state.updated_at_ms ? new Date(state.updated_at_ms).toLocaleTimeString() : "never";
        updateMotionButtons(state.motion_mode);
        updateImuPanel(state.imu);
        updateRobotScene(state.body_scene, state.imu, state.servos ?? []);
        updateManualPanel(state.manual);
        updateArmPanel(state.arm);
        updateTiltedStandPanel(state.tilted_stand);
        updateCalibrationPanel(state.calibration);

        const faulted = state.servos.filter((servo) => servo.telemetry && servo.telemetry.faults.length > 0).length
          + armServos.filter((servo) => servo.telemetry && servo.telemetry.faults.length > 0).length;
        const groupedServos = groupServosByLeg(state.servos);
        const liveLegs = LEG_ORDER.filter((legKey) => groupedServos[legKey].filter((servo) => servo.online).length === 3).length;
        document.getElementById("fault-summary").textContent = armServos.length
          ? `${liveLegs}/${LEG_ORDER.length} legs fully live · arm ${state.arm.online_servo_count}/${armServos.length} live · ${faulted} servo(s) reporting status flags`
          : `${liveLegs}/${LEG_ORDER.length} legs fully live · ${faulted} servo(s) reporting status flags`;
        document.getElementById("rail-leg-summary").textContent = `${liveLegs}/${LEG_ORDER.length} legs fully live`;
        document.getElementById("robot-note").textContent =
          armServos.length
            ? `${state.motion_mode}: ${state.motion_summary} legs ${state.online_servo_count}/${state.servos.length}, arm ${state.arm.online_servo_count}/${armServos.length} joints responding.`
            : `${state.motion_mode}: ${state.motion_summary} ${state.online_servo_count}/${state.servos.length} joints responding.`;
        document.getElementById("rail-leg-previews").innerHTML = renderCompactLegRail(state.servos);
        document.getElementById("servo-map-legs").innerHTML = renderServoMap(state.servos);

        updateBadge(
          totalOnlineServoCount > 0 && !state.motion_fault,
          `${state.robot_name}: ${state.motion_mode}, ${totalOnlineServoCount}/${totalConfiguredServoCount} online`
        );

        if (!streamStarted) {
          const img = document.getElementById("camera-stream");
          document.getElementById("stream-placeholder").hidden = true;
          img.hidden = false;
          img.src = cameraUrl;
          streamStarted = true;
        }
      } catch (error) {
        updateBadge(false, "dashboard fetch error");
        document.getElementById("serial-note").textContent = String(error);
      }
    }

    bindRailVisualTabs();
    setRailVisualTab(railVisualTabState.active);
    refresh();
    setInterval(refresh, 500);
  </script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::DASHBOARD_HTML;

    #[test]
    fn dashboard_html_includes_sticky_side_rail_elements() {
        for needle in [
            "class=\"side-rail\"",
            "id=\"camera-rail-note\"",
            "id=\"rail-leg-previews\"",
            "id=\"rail-imu-summary\"",
            "id=\"robot-scene-view\"",
            "id=\"rail-visual-tab-legs\"",
            "id=\"rail-visual-tab-body\"",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_includes_compact_preview_and_imu_helpers() {
        for needle in [
            "renderCompactLegRail",
            "renderCompactLegPreviewCard",
            "describeImuState",
            "updateRailImuPanel",
            "bindRailVisualTabs",
            "setRailVisualTab",
            "robotSceneLegTelemetryByKey",
            "renderRobotSceneFeedback",
            "ensureRobotSceneRuntime",
            "buildRobotSceneRoot",
            "updateRobotScene",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain helper {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_uses_threejs_orbit_renderer_for_robot_scene() {
        for needle in [
            "<script type=\"importmap\">",
            "three.module.js",
            "three/addons/controls/OrbitControls.js",
            "new THREE_NS.WebGLRenderer",
            "new OrbitControlsCtor",
            "camera.up.set(0, 0, 1);",
            "drag to orbit",
            "double-click to reset",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_defaults_to_body_tab_with_larger_scene() {
        for needle in [
            "const railVisualTabState = { active: \"body\" };",
            "id=\"rail-visual-tab-body\" class=\"rail-tab-btn active\"",
            "id=\"rail-visual-pane-legs\" class=\"rail-tab-pane\" role=\"tabpanel\" aria-labelledby=\"rail-visual-tab-legs\" hidden",
            "min-height: 22.4rem;",
            "height: 22.4rem;",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_robot_scene_integrates_servo_load_feedback() {
        for needle in [
            "id=\"robot-scene-feedback\"",
            "present_load_pct",
            "present_current_ma",
            "max load",
            "Leg segments and glow now reflect live servo load and resistance.",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_robot_scene_tracks_load_per_leg_segment() {
        for needle in [
            "peakJointKey",
            "primaryFaultJointKey",
            "robotSceneJointColor",
            "color.segments[index]",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain per-joint scene feedback fragment {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_robot_scene_uses_direct_ros_z_up_axes() {
        for needle in [
            "return new THREE_NS.Vector3(\n        Number(point?.x ?? 0),\n        Number(point?.y ?? 0),\n        Number(point?.z ?? 0),\n      );",
            "return new THREE_NS.Vector3(forward, left, up);",
            "pitchGroup.rotation.y = -Number(imu?.pitch_deg ?? 0) * Math.PI / 180;",
            "rollGroup.rotation.x = Number(imu?.roll_deg ?? 0) * Math.PI / 180;",
            "grid.rotation.x = Math.PI / 2;",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
        for needle in [
            "-Number(point?.y ?? 0)",
            "rollGroup.rotation.z =",
            "pitchGroup.rotation.x =",
        ] {
            assert!(
                !DASHBOARD_HTML.contains(needle),
                "dashboard html should no longer contain legacy scene basis fragment {needle}"
            );
        }
    }

    #[test]
    fn dashboard_html_includes_wide_two_column_rail_and_body_aligned_leg_glance() {
        for needle in [
            "grid-template-columns: minmax(0, 1fr) clamp(38rem, 46vw, 52rem);",
            "grid-template-columns: repeat(2, minmax(0, 1fr));",
            "class=\"panel rail-panel rail-span-2\"",
            "Leg Visuals",
            "role=\"tablist\"",
            "class=\"rail-leg-body\"",
            "ROS body frame",
            "railClass: \"front-left\"",
            "railClass: \"rear-right\"",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }

    #[test]
    fn dashboard_layout_wraps_side_rail_inside_the_grid_section() {
        let main_column_close = DASHBOARD_HTML
            .find("</section>\n      </div>\n\n      <aside class=\"side-rail\"")
            .expect("main column should close immediately before the side rail");
        let rail_start = DASHBOARD_HTML[main_column_close..]
            .find("<aside class=\"side-rail\"")
            .map(|offset| main_column_close + offset)
            .expect("side rail should exist");
        let layout_end = DASHBOARD_HTML[rail_start..]
            .find("</aside>\n    </section>")
            .map(|offset| rail_start + offset)
            .expect("layout section should close immediately after the side rail");

        assert!(
            layout_end > rail_start,
            "layout section should keep the side rail inside the grid"
        );
    }

    #[test]
    fn dashboard_html_includes_per_slider_torque_controls_for_legs_and_arm() {
        for needle in [
            "id=\"manual-coxa-torque-limit\"",
            "id=\"manual-femur-torque-limit\"",
            "id=\"manual-tibia-torque-limit\"",
            "id=\"arm-${joint.key}-torque-limit\"",
            "const armTorqueLimitUrl = \"/api/arm/torque-limit\";",
            "applyManualAxisTorqueLimit",
            "applyArmJointTorqueLimit",
        ] {
            assert!(
                DASHBOARD_HTML.contains(needle),
                "dashboard html should contain {needle}"
            );
        }
    }
}
