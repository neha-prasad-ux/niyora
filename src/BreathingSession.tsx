import { useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

// --- Config ---
const WIDTH = 300;
const HEIGHT = 380;
const PARTICLE_COUNT = 70;
const SECONDS_PER_PHASE = 4;
const TOTAL_ROUNDS = 4;
const PHASES = ["inhale", "hold-in", "exhale", "hold-out"] as const;
type Phase = (typeof PHASES)[number];

// Chromotherapy color map (HSL)
const PHASE_BG: Record<Phase, [number, number, number]> = {
  "inhale":   [220, 25, 12],  // cool blue-dark
  "hold-in":  [260, 15, 11],  // neutral purple-dark
  "exhale":   [20, 25, 12],   // warm amber-dark
  "hold-out": [260, 15, 11],  // neutral purple-dark
};

const PHASE_LABELS: Record<Phase, string> = {
  "inhale": "inhale",
  "hold-in": "hold",
  "exhale": "exhale",
  "hold-out": "hold",
};

// --- Noise (simple value noise for organic drift) ---
function createNoise() {
  const perm = new Uint8Array(512);
  for (let i = 0; i < 256; i++) perm[i] = i;
  for (let i = 255; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [perm[i], perm[j]] = [perm[j], perm[i]];
  }
  for (let i = 0; i < 256; i++) perm[i + 256] = perm[i];

  function fade(t: number) { return t * t * t * (t * (t * 6 - 15) + 10); }
  function lerp(a: number, b: number, t: number) { return a + t * (b - a); }
  function grad(hash: number, x: number, y: number) {
    const h = hash & 3;
    const u = h < 2 ? x : y;
    const v = h < 2 ? y : x;
    return ((h & 1) ? -u : u) + ((h & 2) ? -v : v);
  }

  return function noise2d(x: number, y: number) {
    const xi = Math.floor(x) & 255;
    const yi = Math.floor(y) & 255;
    const xf = x - Math.floor(x);
    const yf = y - Math.floor(y);
    const u = fade(xf);
    const v = fade(yf);
    const aa = perm[perm[xi] + yi];
    const ab = perm[perm[xi] + yi + 1];
    const ba = perm[perm[xi + 1] + yi];
    const bb = perm[perm[xi + 1] + yi + 1];
    return lerp(
      lerp(grad(aa, xf, yf), grad(ba, xf - 1, yf), u),
      lerp(grad(ab, xf, yf - 1), grad(bb, xf - 1, yf - 1), u),
      v
    );
  };
}

// --- Particle ---
interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  baseSize: number;
  size: number;
  opacity: number;
  noiseOffsetX: number;
  noiseOffsetY: number;
  homeX: number; // scattered home position
  homeY: number;
}

function createParticle(): Particle {
  const angle = Math.random() * Math.PI * 2;
  const dist = 30 + Math.random() * 120;
  const x = WIDTH / 2 + Math.cos(angle) * dist;
  const y = HEIGHT / 2 + Math.sin(angle) * dist;
  return {
    x, y,
    vx: 0, vy: 0,
    baseSize: 1.5 + Math.random() * 1.5,
    size: 1.5 + Math.random() * 1.5,
    opacity: 0.3 + Math.random() * 0.4,
    noiseOffsetX: Math.random() * 1000,
    noiseOffsetY: Math.random() * 1000,
    homeX: x,
    homeY: y,
  };
}

// --- Easing ---
function lerpVal(a: number, b: number, t: number) { return a + (b - a) * t; }
function lerpHSL(
  a: [number, number, number],
  b: [number, number, number],
  t: number
): [number, number, number] {
  // Handle hue wrapping
  let dh = b[0] - a[0];
  if (dh > 180) dh -= 360;
  if (dh < -180) dh += 360;
  return [
    (a[0] + dh * t + 360) % 360,
    lerpVal(a[1], b[1], t),
    lerpVal(a[2], b[2], t),
  ];
}

interface BreathingSessionProps {
  onComplete: () => void;
}

export default function BreathingSession({ onComplete }: BreathingSessionProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef({
    particles: [] as Particle[],
    noise: createNoise(),
    time: 0,
    phase: "inhale" as Phase,
    phaseIndex: 0,
    phaseTime: 0,
    round: 0,
    totalElapsed: 0,
    bgColor: [...PHASE_BG["inhale"]] as [number, number, number],
    targetBgColor: [...PHASE_BG["inhale"]] as [number, number, number],
    done: false,
    doneTime: 0,
    labelOpacity: 0,
  });
  const rafRef = useRef<number>(0);

  const handleDone = useCallback(async () => {
    await invoke("hide_panel");
  }, []);

  // Reset everything when the window becomes visible again
  useEffect(() => {
    function handleVisibility() {
      if (!document.hidden) {
        // Window just became visible — reset state for a fresh session
        const state = stateRef.current;
        state.particles = Array.from({ length: PARTICLE_COUNT }, createParticle);
        state.noise = createNoise();
        state.time = 0;
        state.phase = "inhale";
        state.phaseIndex = 0;
        state.phaseTime = 0;
        state.round = 0;
        state.totalElapsed = 0;
        state.bgColor = [...PHASE_BG["inhale"]];
        state.targetBgColor = [...PHASE_BG["inhale"]];
        state.done = false;
        state.doneTime = 0;
        state.labelOpacity = 0;
      }
    }
    document.addEventListener("visibilitychange", handleVisibility);
    return () => document.removeEventListener("visibilitychange", handleVisibility);
  }, []);

  // Main animation loop (also inits particles)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;

    // Init particles on first mount
    const state = stateRef.current;
    if (state.particles.length === 0) {
      state.particles = Array.from({ length: PARTICLE_COUNT }, createParticle);
    }

    // Set canvas resolution for retina
    const dpr = window.devicePixelRatio || 1;
    canvas.width = WIDTH * dpr;
    canvas.height = HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    let lastTime = performance.now();

    function tick(now: number) {
      const dt = Math.min((now - lastTime) / 1000, 0.05); // cap delta
      lastTime = now;
      const state = stateRef.current;
      state.time += dt;

      if (!state.done) {
        // Advance phase timer
        state.phaseTime += dt;
        state.totalElapsed += dt;

        if (state.phaseTime >= SECONDS_PER_PHASE) {
          state.phaseTime -= SECONDS_PER_PHASE;
          state.phaseIndex++;

          if (state.phaseIndex >= PHASES.length) {
            state.phaseIndex = 0;
            state.round++;
            // round incremented
          }

          if (state.round >= TOTAL_ROUNDS) {
            state.done = true;
            state.doneTime = 0;
            // done
          } else {
            state.phase = PHASES[state.phaseIndex];
            state.targetBgColor = [...PHASE_BG[state.phase]];
            // phase updated
          }
        }
      } else {
        // Completion state
        state.doneTime += dt;
        state.targetBgColor = [30, 15, 10]; // neutral warm
        if (state.doneTime > 5) {
          onComplete();
          return;
        }
      }

      // Smooth background color transition
      state.bgColor = lerpHSL(state.bgColor, state.targetBgColor, dt * 1.2);

      // Phase progress (0 to 1)
      const phaseT = Math.min(state.phaseTime / SECONDS_PER_PHASE, 1);
      const phase = state.phase;
      const cx = WIDTH / 2;
      const cy = HEIGHT / 2;

      // Update particles
      for (const p of state.particles) {
        const noise = state.noise;
        const t = state.time;

        // Noise-based ambient drift (always active)
        const nx = noise(p.noiseOffsetX + t * 0.3, p.noiseOffsetY) * 0.8;
        const ny = noise(p.noiseOffsetX, p.noiseOffsetY + t * 0.3) * 0.8;

        // Direction from particle to center
        const dx = cx - p.x;
        const dy = cy - p.y;
        const dist = Math.sqrt(dx * dx + dy * dy) + 0.1;
        const ndx = dx / dist;
        const ndy = dy / dist;

        let fx = nx;
        let fy = ny;

        if (state.done) {
          // Completion: drift to stillness
          const doneProgress = Math.min(state.doneTime / 3, 1);
          const damping = 0.9 + 0.09 * doneProgress;
          p.vx *= damping;
          p.vy *= damping;
          p.opacity = lerpVal(p.opacity, 0.2, dt * 0.5);
        } else if (phase === "inhale") {
          // Converge toward center — stronger as phase progresses
          const strength = 0.3 + phaseT * 0.7;
          fx += ndx * strength * (1 + 50 / (dist + 20));
          fy += ndy * strength * (1 + 50 / (dist + 20));
          p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.2, dt * 2);
          p.size = lerpVal(p.size, p.baseSize * (1 + phaseT * 0.3), dt * 2);
        } else if (phase === "hold-in") {
          // Hold near center — micro drift only, high opacity
          const holdStrength = 0.1;
          fx += ndx * holdStrength;
          fy += ndy * holdStrength;
          fx *= 0.3;
          fy *= 0.3;
          p.opacity = lerpVal(p.opacity, 0.6 + Math.sin(t * 2 + p.noiseOffsetX) * 0.1, dt * 2);
        } else if (phase === "exhale") {
          // Disperse outward and slightly upward
          const strength = 0.2 + phaseT * 0.6;
          fx -= ndx * strength * (1 + 30 / (dist + 30));
          fy -= ndy * strength * (1 + 30 / (dist + 30));
          fy -= 0.15 * phaseT; // gentle upward drift
          p.opacity = lerpVal(p.opacity, 0.5 - phaseT * 0.2, dt * 2);
          p.size = lerpVal(p.size, p.baseSize * (1 - phaseT * 0.2), dt * 2);
        } else if (phase === "hold-out") {
          // Ambient floating — near-zero velocity
          fx *= 0.4;
          fy *= 0.4;
          // Gently nudge toward home position
          const hx = p.homeX - p.x;
          const hy = p.homeY - p.y;
          fx += hx * 0.01;
          fy += hy * 0.01;
          p.opacity = lerpVal(p.opacity, 0.3 + Math.sin(t * 1.5 + p.noiseOffsetY) * 0.05, dt * 2);
        }

        // Apply forces with high damping
        const damping = 0.92;
        p.vx = p.vx * damping + fx * dt * 8;
        p.vy = p.vy * damping + fy * dt * 8;

        // Clamp velocity
        const maxSpeed = 1.8;
        const speed = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
        if (speed > maxSpeed) {
          p.vx = (p.vx / speed) * maxSpeed;
          p.vy = (p.vy / speed) * maxSpeed;
        }

        p.x += p.vx;
        p.y += p.vy;

        // Soft boundary — nudge back if too far
        const margin = 20;
        if (p.x < -margin) p.vx += 0.5;
        if (p.x > WIDTH + margin) p.vx -= 0.5;
        if (p.y < -margin) p.vy += 0.5;
        if (p.y > HEIGHT + margin) p.vy -= 0.5;
      }

      // --- Render ---
      const [h, s, l] = state.bgColor;
      ctx.fillStyle = `hsl(${h}, ${s}%, ${l}%)`;
      ctx.fillRect(0, 0, WIDTH, HEIGHT);

      // Particles — same hue family, much brighter and more saturated
      for (const p of state.particles) {
        const ph = h + (p.noiseOffsetX % 20 - 10); // slight hue variation
        const ps = Math.min(s + 50, 90);
        const pl = Math.min(l + 50, 78);
        const op = Math.min(p.opacity + 0.25, 0.95);
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op})`;
        ctx.fill();

        // Glow aura
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * 4, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op * 0.12})`;
        ctx.fill();
      }

      // Phase label — small, centered, low opacity
      const labelText = state.done ? "done" : PHASE_LABELS[state.phase];
      const targetLabelOpacity = state.done
        ? (state.doneTime > 0.5 ? 0.55 : 0)
        : 0.5;
      state.labelOpacity = lerpVal(state.labelOpacity, targetLabelOpacity, dt * 3);

      ctx.font = "300 15px -apple-system, BlinkMacSystemFont, sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillStyle = `rgba(255, 255, 255, ${state.labelOpacity})`;
      ctx.fillText(labelText, cx, cy);

      // Round dots at bottom
      if (!state.done) {
        const dotY = HEIGHT - 24;
        const dotSpacing = 12;
        const startX = cx - ((TOTAL_ROUNDS - 1) * dotSpacing) / 2;
        for (let i = 0; i < TOTAL_ROUNDS; i++) {
          const dx = startX + i * dotSpacing;
          ctx.beginPath();
          ctx.arc(dx, dotY, 3, 0, Math.PI * 2);
          if (i < state.round) {
            ctx.fillStyle = `hsla(${h + 40}, ${s + 20}%, ${l + 25}%, 0.5)`;
          } else if (i === state.round) {
            ctx.fillStyle = `hsla(${h + 40}, ${s + 20}%, ${l + 25}%, 0.35)`;
          } else {
            ctx.fillStyle = `rgba(255, 255, 255, 0.1)`;
          }
          ctx.fill();
        }
      }

      rafRef.current = requestAnimationFrame(tick);
    }

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [onComplete]);

  return (
    <div
      style={{
        width: WIDTH,
        height: HEIGHT,
        borderRadius: 12,
        overflow: "hidden",
        position: "relative",
        cursor: "default",
        userSelect: "none",
      }}
    >
      <canvas
        ref={canvasRef}
        style={{ width: WIDTH, height: HEIGHT, display: "block" }}
      />
      {/* Close X button */}
      <button
        onClick={handleDone}
        title="Close"
        style={{
          position: "absolute",
          top: 8,
          right: 8,
          width: 26,
          height: 26,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "transparent",
          border: "none",
          color: "rgba(255,255,255,0.2)",
          cursor: "pointer",
          borderRadius: 6,
          zIndex: 10,
          transition: "all 0.2s ease",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.color = "rgba(255,255,255,0.5)";
          e.currentTarget.style.background = "rgba(255,255,255,0.08)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = "rgba(255,255,255,0.2)";
          e.currentTarget.style.background = "transparent";
        }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
          <path d="M2 2l8 8M10 2l-8 8" />
        </svg>
      </button>
    </div>
  );
}
