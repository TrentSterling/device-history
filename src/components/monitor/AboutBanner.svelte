<div class="about-banner">
  <div class="banner-content">
    <span class="tagline">⚡ WTF just disconnected?</span>
    <span class="dash">—</span>
    <span class="desc">Real-time USB monitor</span>
    <div class="features">
      <span class="feature-pill live">🟢 Live</span>
      <span class="feature-pill">📝 Log</span>
      <span class="feature-pill">💾 Cache</span>
      <span class="feature-pill">⌨️ CLI</span>
      <span class="feature-pill">🎨 5 Themes</span>
    </div>
  </div>
  <div class="banner-glow"></div>
</div>

<style>
  .about-banner {
    display: flex;
    align-items: center;
    padding: 10px 14px;
    border-radius: 10px;
    border: 1px solid var(--glass-border);
    background: var(--glass-bg);
    backdrop-filter: blur(12px) saturate(1.3);
    position: relative;
    overflow: hidden;
    box-shadow: 0 2px 12px var(--shadow-color);
  }
  .banner-glow {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      var(--accent-glow),
      color-mix(in srgb, var(--cyan) 8%, transparent),
      var(--accent-glow),
      transparent
    );
    background-size: 300% 100%;
    animation: gradientShift 8s ease infinite;
    pointer-events: none;
  }
  .banner-content {
    display: flex;
    align-items: center;
    gap: 10px;
    position: relative;
    z-index: 1;
    flex-wrap: wrap;
  }
  .tagline {
    font-size: 14px;
    font-weight: 700;
    background: linear-gradient(135deg, var(--accent), var(--pink));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  .dash {
    font-size: 11px;
    color: var(--text-muted);
  }
  .desc {
    font-size: 12px;
    color: var(--text-sec);
  }
  .features {
    display: flex;
    gap: 6px;
    margin-left: 4px;
    flex-wrap: wrap;
  }
  .feature-pill {
    font-size: 10px;
    color: var(--text-sec);
    padding: 2px 8px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--bg-elevated) 80%, transparent);
    border: 1px solid var(--border);
    white-space: nowrap;
    transition: all 180ms ease;
  }
  .feature-pill:hover {
    border-color: var(--accent);
    color: var(--text);
    box-shadow: 0 0 6px var(--accent-glow);
  }
  .feature-pill.live {
    border-color: color-mix(in srgb, var(--green) 30%, var(--border));
    color: var(--green);
  }
</style>
