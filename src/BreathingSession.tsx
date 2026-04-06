import { useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { randomTechnique, type Technique, type VisualConfig } from "./techniques";

// --- Config ---
const WIDTH = 300;
const HEIGHT = 380;
const PARTICLE_COUNT = 70;
const INTRO_DURATION = 3;

// --- Noise ---
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
      lerp(grad(ab, xf, yf - 1), grad(bb, xf - 1, yf - 1), u), v
    );
  };
}

// --- Particle ---
interface Particle {
  x: number; y: number; vx: number; vy: number;
  baseSize: number; size: number; opacity: number;
  noiseOffsetX: number; noiseOffsetY: number;
  homeX: number; homeY: number;
  side: number; // -1 = left, 1 = right (for alternate nostril)
}

function createParticle(): Particle {
  const angle = Math.random() * Math.PI * 2;
  const dist = 30 + Math.random() * 120;
  const x = WIDTH / 2 + Math.cos(angle) * dist;
  const y = HEIGHT / 2 + Math.sin(angle) * dist;
  return {
    x, y, vx: 0, vy: 0,
    baseSize: 1.5 + Math.random() * 1.5,
    size: 1.5 + Math.random() * 1.5,
    opacity: 0.3 + Math.random() * 0.4,
    noiseOffsetX: Math.random() * 1000,
    noiseOffsetY: Math.random() * 1000,
    homeX: x, homeY: y,
    side: x < WIDTH / 2 ? -1 : 1,
  };
}

// --- Helpers ---
function lerpVal(a: number, b: number, t: number) { return a + (b - a) * t; }
function lerpHSL(
  a: [number, number, number], b: [number, number, number], t: number
): [number, number, number] {
  let dh = b[0] - a[0];
  if (dh > 180) dh -= 360;
  if (dh < -180) dh += 360;
  return [(a[0] + dh * t + 360) % 360, lerpVal(a[1], b[1], t), lerpVal(a[2], b[2], t)];
}

// =====================================================================
// MOTION FUNCTIONS — each returns (fx, fy) force + mutates particle
// =====================================================================
function motionConverge(
  p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number
) {
  let fx = nx, fy = ny;
  if (phaseType === "inhale") {
    const str = 0.3 + phaseT * 0.7;
    fx += ndx * str * (1 + 50 / (dist + 20));
    fy += ndy * str * (1 + 50 / (dist + 20));
    p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 + phaseT * 0.3), dt * 2);
  } else if (phaseType === "hold") {
    fx += ndx * 0.1; fy += ndy * 0.1;
    fx *= 0.3; fy *= 0.3;
    p.opacity = lerpVal(p.opacity, 0.6 + Math.sin(t * 2 + p.noiseOffsetX) * 0.1, dt * 2);
  } else {
    const str = 0.2 + phaseT * 0.6;
    fx -= ndx * str * (1 + 30 / (dist + 30));
    fy -= ndy * str * (1 + 30 / (dist + 30));
    fy -= 0.15 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.5 - phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 - phaseT * 0.2), dt * 2);
  }
  return { fx, fy };
}

function motionWave(
  p: Particle, _ndx: number, _ndy: number, _dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number
) {
  let fx = nx * 0.5, fy = ny * 0.3;
  // Rolling horizontal wave — particles follow sine wave
  const wavePhase = p.x / WIDTH * Math.PI * 2 + t * 0.8;
  const waveY = Math.sin(wavePhase) * 0.6;
  fy += waveY;

  if (phaseType === "inhale") {
    // Waves gather toward center height
    fy += (HEIGHT / 2 - p.y) * 0.005 * (1 + phaseT);
    p.opacity = lerpVal(p.opacity, 0.55 + phaseT * 0.15, dt * 2);
  } else if (phaseType === "exhale") {
    // Waves spread vertically
    fy += (p.homeY - p.y) * 0.003;
    fx += Math.cos(t + p.noiseOffsetX) * 0.3;
    p.opacity = lerpVal(p.opacity, 0.4 - phaseT * 0.1, dt * 2);
  } else {
    fx *= 0.3; fy *= 0.3;
    p.opacity = lerpVal(p.opacity, 0.5, dt * 2);
  }
  return { fx, fy };
}

function motionSnowfall(
  p: Particle, _ndx: number, _ndy: number, _dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, _ny: number
) {
  let fx = nx * 0.4, fy = 0;
  // Gentle downward drift like snow
  fy += 0.3 + Math.sin(t * 0.5 + p.noiseOffsetX) * 0.15;
  fx += Math.sin(t * 0.3 + p.noiseOffsetY) * 0.2; // gentle lateral sway

  if (phaseType === "inhale") {
    fy += 0.2 * phaseT; // drift down faster as inhaling through mouth
    p.opacity = lerpVal(p.opacity, 0.6 + phaseT * 0.15, dt * 2);
    // Sparkle: occasional brightness pulse
    if (Math.sin(t * 5 + p.noiseOffsetX * 10) > 0.7) {
      p.opacity = Math.min(p.opacity + 0.15, 0.9);
    }
  } else if (phaseType === "exhale") {
    fy -= 0.1; // slow the descent
    fx += (WIDTH / 2 - p.x) * 0.002; // drift toward center
    p.opacity = lerpVal(p.opacity, 0.35, dt * 2);
  } else {
    fy *= 0.3;
    p.opacity = lerpVal(p.opacity, 0.45, dt * 2);
  }

  // Wrap around: reappear at top if fallen below
  if (p.y > HEIGHT + 20) { p.y = -10; p.x = Math.random() * WIDTH; }

  return { fx, fy };
}

function motionAlternate(
  p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number, phaseLabel: string
) {
  let fx = nx * 0.5, fy = ny * 0.5;
  const cx = WIDTH / 2;
  const isLeftPhase = phaseLabel.includes("left");
  const isRightPhase = phaseLabel.includes("right");

  if (phaseType === "inhale") {
    // Active side converges, other side drifts gently
    const targetX = isLeftPhase ? cx - 50 : cx + 50;
    const active = isLeftPhase ? (p.side === -1) : (p.side === 1);
    if (active) {
      fx += (targetX - p.x) * 0.01 * (1 + phaseT);
      fy += (HEIGHT / 2 - p.y) * 0.005 * (1 + phaseT);
      p.opacity = lerpVal(p.opacity, 0.6 + phaseT * 0.15, dt * 2);
    } else {
      fx *= 0.2; fy *= 0.2;
      p.opacity = lerpVal(p.opacity, 0.25, dt * 2);
    }
  } else if (phaseType === "exhale") {
    const active = isRightPhase ? (p.side === 1) : (p.side === -1);
    if (active) {
      fx -= ndx * 0.3 * (1 + phaseT);
      fy -= ndy * 0.3 * (1 + phaseT);
      p.opacity = lerpVal(p.opacity, 0.5 - phaseT * 0.15, dt * 2);
    } else {
      fx *= 0.2; fy *= 0.2;
      p.opacity = lerpVal(p.opacity, 0.25, dt * 2);
    }
  } else {
    fx += ndx * 0.05; fy += ndy * 0.05;
    fx *= 0.3; fy *= 0.3;
    p.opacity = lerpVal(p.opacity, 0.45, dt * 2);
  }
  return { fx, fy };
}

function motionLunar(
  p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number
) {
  let fx = nx * 0.4, fy = ny * 0.4;
  // Everything drifts gently leftward — lunar, dreamy
  fx -= 0.15;
  fy += Math.sin(t * 0.4 + p.noiseOffsetX) * 0.1;

  if (phaseType === "inhale") {
    fx += ndx * 0.2 * (1 + phaseT * 0.5);
    fy += ndy * 0.2 * (1 + phaseT * 0.5);
    p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.15, dt * 1.5);
  } else if (phaseType === "exhale") {
    fx -= 0.2 * phaseT; // drift further left
    fy -= 0.05;
    p.opacity = lerpVal(p.opacity, 0.35 - phaseT * 0.1, dt * 1.5);
  } else {
    fx *= 0.2; fy *= 0.2;
    p.opacity = lerpVal(p.opacity, 0.4, dt * 2);
  }

  // Soft wrap: reappear on right if drifted too far left
  if (p.x < -30) { p.x = WIDTH + 10; }

  return { fx, fy };
}

function motionBelly(
  p: Particle, _ndx: number, _ndy: number, _dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number
) {
  let fx = nx * 0.3, fy = ny * 0.3;
  const cy = HEIGHT / 2;

  if (phaseType === "inhale") {
    // Particles rise upward (belly expanding)
    fy -= 0.4 * (1 + phaseT * 0.5);
    // Spread outward horizontally
    fx += (p.x - WIDTH / 2) * 0.003 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 + phaseT * 0.4), dt * 2);
  } else if (phaseType === "exhale") {
    // Particles sink downward (belly releasing)
    fy += 0.3 * (1 + phaseT * 0.3);
    // Contract horizontally
    fx += (WIDTH / 2 - p.x) * 0.002 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.45 - phaseT * 0.1, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 - phaseT * 0.2), dt * 2);
  } else {
    fy += (cy - p.y) * 0.003;
    p.opacity = lerpVal(p.opacity, 0.45, dt * 2);
  }
  return { fx, fy };
}

function motionSedation(
  p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, t: number, dt: number,
  nx: number, ny: number, _label: string, roundProgress: number
) {
  // roundProgress: 0 to 1 over all rounds — particles slow and dim over time
  const slowdown = 1 - roundProgress * 0.6; // goes from 1.0 to 0.4
  let fx = nx * 0.5 * slowdown, fy = ny * 0.5 * slowdown;

  if (phaseType === "inhale") {
    const str = (0.2 + phaseT * 0.4) * slowdown;
    fx += ndx * str * (1 + 40 / (dist + 25));
    fy += ndy * str * (1 + 40 / (dist + 25));
    p.opacity = lerpVal(p.opacity, (0.5 + phaseT * 0.15) * (1 - roundProgress * 0.3), dt * 2);
  } else if (phaseType === "hold") {
    fx *= 0.2; fy *= 0.2;
    p.opacity = lerpVal(p.opacity, (0.55) * (1 - roundProgress * 0.3), dt * 1.5);
  } else {
    const str = (0.15 + phaseT * 0.3) * slowdown;
    fx -= ndx * str * (1 + 20 / (dist + 30));
    fy -= ndy * str * (1 + 20 / (dist + 30));
    p.opacity = lerpVal(p.opacity, (0.4 - phaseT * 0.1) * (1 - roundProgress * 0.4), dt * 2);
  }
  return { fx, fy };
}

// =====================================================================
// COMPONENT
// =====================================================================
interface AnimState {
  particles: Particle[];
  noise: ReturnType<typeof createNoise>;
  technique: Technique;
  time: number;
  introTime: number; introDone: boolean;
  phaseIndex: number; phaseTime: number; round: number;
  done: boolean; doneTime: number;
  bgColor: [number, number, number];
  targetBgColor: [number, number, number];
  labelOpacity: number; nameOpacity: number; subtitleOpacity: number;
}

function createState(): AnimState {
  const technique = randomTechnique();
  return {
    particles: Array.from({ length: PARTICLE_COUNT }, createParticle),
    noise: createNoise(),
    technique,
    time: 0,
    introTime: 0, introDone: false,
    phaseIndex: 0, phaseTime: 0, round: 0,
    done: false, doneTime: 0,
    bgColor: [260, 15, 11],
    targetBgColor: [260, 15, 11],
    labelOpacity: 0, nameOpacity: 0, subtitleOpacity: 0,
  };
}

interface Props { onComplete: () => void; }

export default function BreathingSession({ onComplete }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef<AnimState>(createState());
  const resetFlag = useRef(0);
  const rafRef = useRef<number>(0);

  const handleDone = useCallback(async () => { await invoke("hide_panel"); }, []);

  // Register global reset function called by Rust via window.eval
  useEffect(() => {
    (window as any).__NIYORA_RESET = () => {
      resetFlag.current++;
    };
    return () => { delete (window as any).__NIYORA_RESET; };
  }, []);

  // Animation loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = WIDTH * dpr;
    canvas.height = HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    let lastTime = performance.now();
    let lastResetFlag = resetFlag.current;

    function tick(now: number) {
      // Check if reset was requested
      if (resetFlag.current !== lastResetFlag) {
        lastResetFlag = resetFlag.current;
        stateRef.current = createState();
        lastTime = now;
      }

      const dt = Math.min((now - lastTime) / 1000, 0.05);
      lastTime = now;
      const s = stateRef.current;
      s.time += dt;

      const technique = s.technique;
      const visual = technique.visual;
      const cx = WIDTH / 2, cy = HEIGHT / 2;

      // ---- INTRO ----
      if (!s.introDone) {
        s.introTime += dt;
        if (s.introTime < 1) {
          s.nameOpacity = lerpVal(s.nameOpacity, 0.7, dt * 4);
          s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0.35, dt * 3);
        } else if (s.introTime < 2) {
          s.nameOpacity = 0.7; s.subtitleOpacity = 0.35;
        } else {
          s.nameOpacity = lerpVal(s.nameOpacity, 0, dt * 4);
          s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0, dt * 4);
        }
        if (s.introTime >= INTRO_DURATION) {
          s.introDone = true;
          s.nameOpacity = 0; s.subtitleOpacity = 0;
          s.targetBgColor = [...visual.colors[technique.phases[0].type]];
        }
        s.targetBgColor = [260, 15, 11];
      }

      // ---- BREATHING ----
      let currentPhaseType = "hold";
      let currentLabel = "";
      let phaseDuration = 4;
      let phaseT = 0;

      if (s.introDone && !s.done) {
        const phase = technique.phases[s.phaseIndex];
        currentPhaseType = phase.type;
        currentLabel = phase.label;
        phaseDuration = phase.duration;
        s.phaseTime += dt;
        phaseT = Math.min(s.phaseTime / phaseDuration, 1);

        if (s.phaseTime >= phaseDuration) {
          s.phaseTime -= phaseDuration;
          s.phaseIndex++;
          if (s.phaseIndex >= technique.phases.length) { s.phaseIndex = 0; s.round++; }
          if (s.round >= technique.rounds) { s.done = true; s.doneTime = 0; }
          else { s.targetBgColor = [...visual.colors[technique.phases[s.phaseIndex].type]]; }
        }
      }

      // ---- COMPLETION ----
      if (s.done) {
        s.doneTime += dt;
        // Warm golden glow background, then slowly dim
        if (s.doneTime < 1.5) {
          s.targetBgColor = [35, 30, 14]; // warm golden
        } else {
          s.targetBgColor = [30, 15, 10]; // fade to neutral
        }
        if (s.doneTime > 7) { onComplete(); return; }
      }

      s.bgColor = lerpHSL(s.bgColor, s.targetBgColor, dt * 1.2);

      // Round progress for sedation mode
      const roundProgress = technique.rounds > 0 ? s.round / technique.rounds : 0;

      // ---- UPDATE PARTICLES ----
      for (const p of s.particles) {
        const t = s.time;
        const nx = s.noise(p.noiseOffsetX + t * 0.3, p.noiseOffsetY) * 0.8;
        const ny = s.noise(p.noiseOffsetX, p.noiseOffsetY + t * 0.3) * 0.8;

        const dx = cx - p.x, dy = cy - p.y;
        const dist = Math.sqrt(dx * dx + dy * dy) + 0.1;
        const ndx = dx / dist, ndy = dy / dist;

        let force: { fx: number; fy: number };

        if (s.done) {
          const doneT = s.doneTime;
          if (doneT < 1.2) {
            // Phase 1: Celebration burst — particles push outward with energy
            const burstStrength = (1.2 - doneT) / 1.2; // fades from 1 to 0
            force = {
              fx: -ndx * 2.5 * burstStrength + nx * 0.5,
              fy: -ndy * 2.5 * burstStrength + ny * 0.5 - 0.3 * burstStrength,
            };
            // Brighten during burst
            p.opacity = lerpVal(p.opacity, 0.7 + burstStrength * 0.25, dt * 5);
            p.size = lerpVal(p.size, p.baseSize * (1.3 + burstStrength * 0.5), dt * 4);
          } else if (doneT < 3.5) {
            // Phase 2: Gentle orbit — particles slowly swirl
            const orbitT = (doneT - 1.2) / 2.3;
            const angle = Math.atan2(p.y - cy, p.x - cx);
            force = {
              fx: Math.cos(angle + Math.PI / 2) * 0.3 * (1 - orbitT) + nx * 0.2,
              fy: Math.sin(angle + Math.PI / 2) * 0.3 * (1 - orbitT) + ny * 0.2,
            };
            p.opacity = lerpVal(p.opacity, 0.5 - orbitT * 0.15, dt * 1.5);
            p.size = lerpVal(p.size, p.baseSize * 1.1, dt * 2);
          } else {
            // Phase 3: Drift to stillness
            const fadeT = Math.min((doneT - 3.5) / 3, 1);
            p.vx *= 0.92 + 0.07 * fadeT;
            p.vy *= 0.92 + 0.07 * fadeT;
            p.opacity = lerpVal(p.opacity, 0.15 * (1 - fadeT), dt * 1);
            force = { fx: nx * 0.1, fy: ny * 0.1 };
          }
        } else if (!s.introDone) {
          force = { fx: nx * 0.3, fy: ny * 0.3 };
          p.opacity = lerpVal(p.opacity, 0.35, dt * 2);
        } else {
          // Dispatch to motion function
          switch (visual.motion) {
            case "wave":
              force = motionWave(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
              break;
            case "snowfall":
              force = motionSnowfall(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
              break;
            case "alternate":
              force = motionAlternate(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny, currentLabel);
              break;
            case "lunar":
              force = motionLunar(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
              break;
            case "belly":
              force = motionBelly(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
              break;
            case "sedation":
              force = motionSedation(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny, currentLabel, roundProgress);
              break;
            default:
              force = motionConverge(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
          }
        }

        p.vx = p.vx * 0.92 + force.fx * dt * 8;
        p.vy = p.vy * 0.92 + force.fy * dt * 8;

        const maxSpeed = 1.8;
        const spd = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
        if (spd > maxSpeed) { p.vx = (p.vx / spd) * maxSpeed; p.vy = (p.vy / spd) * maxSpeed; }

        p.x += p.vx; p.y += p.vy;

        const margin = 20;
        if (p.x < -margin) p.vx += 0.5;
        if (p.x > WIDTH + margin) p.vx -= 0.5;
        if (p.y < -margin) p.vy += 0.5;
        if (p.y > HEIGHT + margin) p.vy -= 0.5;
      }

      // ---- RENDER ----
      const [h, sat, l] = s.bgColor;
      ctx.fillStyle = `hsl(${h}, ${sat}%, ${l}%)`;
      ctx.fillRect(0, 0, WIDTH, HEIGHT);

      const boost = visual.brightnessBoost || 0;
      for (const p of s.particles) {
        const ph = h + (p.noiseOffsetX % 20 - 10);
        const ps = Math.min(sat + 50, 90);
        const pl = Math.min(l + 50 + boost * 100, 85);
        const op = Math.min(p.opacity + 0.25, 0.95);
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op})`;
        ctx.fill();
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * 4, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op * 0.12})`;
        ctx.fill();
      }

      // ---- TEXT ----
      ctx.textAlign = "center"; ctx.textBaseline = "middle";

      if (!s.introDone) {
        ctx.font = "300 22px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.nameOpacity})`;
        ctx.fillText(technique.name, cx, cy - 14);
        ctx.font = "300 13px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.subtitleOpacity})`;
        ctx.fillText(technique.subtitle, cx, cy + 14);
      } else if (s.done) {
        const doneT = s.doneTime;
        // Fade in after burst
        const targetOp = doneT > 1.0 ? 0.65 : 0;
        s.labelOpacity = lerpVal(s.labelOpacity, targetOp, dt * 2.5);
        // Fade out near the end
        if (doneT > 5.5) {
          s.labelOpacity = lerpVal(s.labelOpacity, 0, dt * 3);
        }

        // "well done" in warm white
        ctx.font = "200 20px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 245, 230, ${s.labelOpacity})`;
        ctx.fillText("well done", cx, cy - 8);

        // Subtle subtitle
        if (doneT > 1.8) {
          const subOp = Math.min((doneT - 1.8) * 0.8, 0.3);
          const subOpFinal = doneT > 5.5 ? lerpVal(subOp, 0, (doneT - 5.5) * 2) : subOp;
          ctx.font = "300 12px -apple-system, BlinkMacSystemFont, sans-serif";
          ctx.fillStyle = `rgba(255, 245, 230, ${Math.max(0, subOpFinal)})`;
          ctx.fillText("take this calm with you", cx, cy + 16);
        }
      } else {
        s.labelOpacity = lerpVal(s.labelOpacity, 0.5, dt * 3);
        ctx.font = "300 15px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity})`;
        ctx.fillText(currentLabel, cx, cy);
      }

      // Round dots
      if (s.introDone && !s.done) {
        const dotY = HEIGHT - 24;
        const dotSpacing = 12;
        const total = technique.rounds;
        const startX = cx - ((total - 1) * dotSpacing) / 2;
        for (let i = 0; i < total; i++) {
          ctx.beginPath();
          ctx.arc(startX + i * dotSpacing, dotY, 3, 0, Math.PI * 2);
          ctx.fillStyle = i < s.round
            ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.5)`
            : i === s.round
            ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.35)`
            : `rgba(255, 255, 255, 0.1)`;
          ctx.fill();
        }
      }

      rafRef.current = requestAnimationFrame(tick);
    }

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [onComplete]);

  return (
    <div style={{
      width: WIDTH, height: HEIGHT, borderRadius: 12,
      overflow: "hidden", position: "relative",
      cursor: "default", userSelect: "none",
    }}>
      <canvas ref={canvasRef} style={{ width: WIDTH, height: HEIGHT, display: "block" }} />
      <button onClick={handleDone} title="Close" style={{
        position: "absolute", top: 8, right: 8, width: 26, height: 26,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: "transparent", border: "none", color: "rgba(255,255,255,0.2)",
        cursor: "pointer", borderRadius: 6, zIndex: 10, transition: "all 0.2s ease",
        outline: "none",
      }}
        onMouseEnter={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.5)"; e.currentTarget.style.background = "rgba(255,255,255,0.08)"; }}
        onMouseLeave={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.2)"; e.currentTarget.style.background = "transparent"; }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
          <path d="M2 2l8 8M10 2l-8 8" />
        </svg>
      </button>
    </div>
  );
}
