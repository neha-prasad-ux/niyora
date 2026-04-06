import { useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  randomTechnique,
  type Technique,
  type BreathingTechnique,
  type MindfulnessTechnique,
  type VisualConfig,
} from "./techniques";

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
    const xi = Math.floor(x) & 255; const yi = Math.floor(y) & 255;
    const xf = x - Math.floor(x); const yf = y - Math.floor(y);
    const u = fade(xf); const v = fade(yf);
    const aa = perm[perm[xi] + yi]; const ab = perm[perm[xi] + yi + 1];
    const ba = perm[perm[xi + 1] + yi]; const bb = perm[perm[xi + 1] + yi + 1];
    return lerp(lerp(grad(aa, xf, yf), grad(ba, xf - 1, yf), u),
      lerp(grad(ab, xf, yf - 1), grad(bb, xf - 1, yf - 1), u), v);
  };
}

// --- Particle ---
interface Particle {
  x: number; y: number; vx: number; vy: number;
  baseSize: number; size: number; opacity: number;
  noiseOffsetX: number; noiseOffsetY: number;
  homeX: number; homeY: number;
  side: number;
}

function createParticle(): Particle {
  const angle = Math.random() * Math.PI * 2;
  const dist = 30 + Math.random() * 120;
  const x = WIDTH / 2 + Math.cos(angle) * dist;
  const y = HEIGHT / 2 + Math.sin(angle) * dist;
  return {
    x, y, vx: 0, vy: 0,
    baseSize: 1.5 + Math.random() * 1.5, size: 1.5 + Math.random() * 1.5,
    opacity: 0.3 + Math.random() * 0.4,
    noiseOffsetX: Math.random() * 1000, noiseOffsetY: Math.random() * 1000,
    homeX: x, homeY: y, side: x < WIDTH / 2 ? -1 : 1,
  };
}

// --- Helpers ---
function lerpVal(a: number, b: number, t: number) { return a + (b - a) * t; }
function lerpHSL(a: [number, number, number], b: [number, number, number], t: number): [number, number, number] {
  let dh = b[0] - a[0]; if (dh > 180) dh -= 360; if (dh < -180) dh += 360;
  return [(a[0] + dh * t + 360) % 360, lerpVal(a[1], b[1], t), lerpVal(a[2], b[2], t)];
}

// Word-wrap for canvas text
function wrapText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
  const words = text.split(' ');
  const lines: string[] = [];
  let currentLine = words[0];
  for (let i = 1; i < words.length; i++) {
    const test = currentLine + ' ' + words[i];
    if (ctx.measureText(test).width < maxWidth) { currentLine = test; }
    else { lines.push(currentLine); currentLine = words[i]; }
  }
  lines.push(currentLine);
  return lines;
}

// =====================================================================
// MOTION FUNCTIONS
// =====================================================================
type ForceResult = { fx: number; fy: number };

function motionConverge(p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  let fx = nx, fy = ny;
  if (phaseType === "inhale") {
    const str = 0.3 + phaseT * 0.7;
    fx += ndx * str * (1 + 50 / (dist + 20)); fy += ndy * str * (1 + 50 / (dist + 20));
    p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 + phaseT * 0.3), dt * 2);
  } else if (phaseType === "hold") {
    fx += ndx * 0.1; fy += ndy * 0.1; fx *= 0.3; fy *= 0.3;
    p.opacity = lerpVal(p.opacity, 0.6 + Math.sin(t * 2 + p.noiseOffsetX) * 0.1, dt * 2);
  } else {
    const str = 0.2 + phaseT * 0.6;
    fx -= ndx * str * (1 + 30 / (dist + 30)); fy -= ndy * str * (1 + 30 / (dist + 30));
    fy -= 0.15 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.5 - phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 - phaseT * 0.2), dt * 2);
  }
  return { fx, fy };
}

function motionWave(p: Particle, _n: number, _n2: number, _d: number,
  phaseType: string, phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  let fx = nx * 0.5, fy = ny * 0.3;
  fy += Math.sin(p.x / WIDTH * Math.PI * 2 + t * 0.8) * 0.6;
  if (phaseType === "inhale") {
    fy += (HEIGHT / 2 - p.y) * 0.005 * (1 + phaseT);
    p.opacity = lerpVal(p.opacity, 0.55 + phaseT * 0.15, dt * 2);
  } else if (phaseType === "exhale") {
    fy += (p.homeY - p.y) * 0.003; fx += Math.cos(t + p.noiseOffsetX) * 0.3;
    p.opacity = lerpVal(p.opacity, 0.4 - phaseT * 0.1, dt * 2);
  } else { fx *= 0.3; fy *= 0.3; p.opacity = lerpVal(p.opacity, 0.5, dt * 2); }
  return { fx, fy };
}

function motionSnowfall(p: Particle, _n: number, _n2: number, _d: number,
  phaseType: string, phaseT: number, t: number, dt: number, nx: number): ForceResult {
  let fx = nx * 0.4, fy = 0.3 + Math.sin(t * 0.5 + p.noiseOffsetX) * 0.15;
  fx += Math.sin(t * 0.3 + p.noiseOffsetY) * 0.2;
  if (phaseType === "inhale") {
    fy += 0.2 * phaseT; p.opacity = lerpVal(p.opacity, 0.6 + phaseT * 0.15, dt * 2);
    if (Math.sin(t * 5 + p.noiseOffsetX * 10) > 0.7) p.opacity = Math.min(p.opacity + 0.15, 0.9);
  } else if (phaseType === "exhale") {
    fy -= 0.1; fx += (WIDTH / 2 - p.x) * 0.002; p.opacity = lerpVal(p.opacity, 0.35, dt * 2);
  } else { fy *= 0.3; p.opacity = lerpVal(p.opacity, 0.45, dt * 2); }
  if (p.y > HEIGHT + 20) { p.y = -10; p.x = Math.random() * WIDTH; }
  return { fx, fy };
}

function motionAlternate(p: Particle, ndx: number, ndy: number, _d: number,
  phaseType: string, phaseT: number, _t: number, dt: number, nx: number, ny: number, label: string): ForceResult {
  let fx = nx * 0.5, fy = ny * 0.5;
  const cx = WIDTH / 2;
  const isLeft = label.includes("left"); const isRight = label.includes("right");
  if (phaseType === "inhale") {
    const targetX = isLeft ? cx - 50 : cx + 50;
    const active = isLeft ? (p.side === -1) : (p.side === 1);
    if (active) { fx += (targetX - p.x) * 0.01 * (1 + phaseT); fy += (HEIGHT / 2 - p.y) * 0.005 * (1 + phaseT); p.opacity = lerpVal(p.opacity, 0.6 + phaseT * 0.15, dt * 2); }
    else { fx *= 0.2; fy *= 0.2; p.opacity = lerpVal(p.opacity, 0.25, dt * 2); }
  } else if (phaseType === "exhale") {
    const active = isRight ? (p.side === 1) : (p.side === -1);
    if (active) { fx -= ndx * 0.3 * (1 + phaseT); fy -= ndy * 0.3 * (1 + phaseT); p.opacity = lerpVal(p.opacity, 0.5 - phaseT * 0.15, dt * 2); }
    else { fx *= 0.2; fy *= 0.2; p.opacity = lerpVal(p.opacity, 0.25, dt * 2); }
  } else { fx *= 0.3; fy *= 0.3; p.opacity = lerpVal(p.opacity, 0.45, dt * 2); }
  return { fx, fy };
}

function motionLunar(p: Particle, ndx: number, ndy: number, _d: number,
  phaseType: string, phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  let fx = nx * 0.4 - 0.15, fy = ny * 0.4 + Math.sin(t * 0.4 + p.noiseOffsetX) * 0.1;
  if (phaseType === "inhale") { fx += ndx * 0.2 * (1 + phaseT * 0.5); fy += ndy * 0.2 * (1 + phaseT * 0.5); p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.15, dt * 1.5); }
  else if (phaseType === "exhale") { fx -= 0.2 * phaseT; fy -= 0.05; p.opacity = lerpVal(p.opacity, 0.35 - phaseT * 0.1, dt * 1.5); }
  else { fx *= 0.2; fy *= 0.2; p.opacity = lerpVal(p.opacity, 0.4, dt * 2); }
  if (p.x < -30) p.x = WIDTH + 10;
  return { fx, fy };
}

function motionBelly(p: Particle, _n: number, _n2: number, _d: number,
  phaseType: string, phaseT: number, _t: number, dt: number, nx: number, ny: number): ForceResult {
  let fx = nx * 0.3, fy = ny * 0.3;
  if (phaseType === "inhale") {
    fy -= 0.4 * (1 + phaseT * 0.5); fx += (p.x - WIDTH / 2) * 0.003 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.5 + phaseT * 0.2, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 + phaseT * 0.4), dt * 2);
  } else if (phaseType === "exhale") {
    fy += 0.3 * (1 + phaseT * 0.3); fx += (WIDTH / 2 - p.x) * 0.002 * phaseT;
    p.opacity = lerpVal(p.opacity, 0.45 - phaseT * 0.1, dt * 2);
    p.size = lerpVal(p.size, p.baseSize * (1 - phaseT * 0.2), dt * 2);
  } else { fy += (HEIGHT / 2 - p.y) * 0.003; p.opacity = lerpVal(p.opacity, 0.45, dt * 2); }
  return { fx, fy };
}

function motionSedation(p: Particle, ndx: number, ndy: number, dist: number,
  phaseType: string, phaseT: number, _t: number, dt: number, nx: number, ny: number, _l: string, rp: number): ForceResult {
  const slow = 1 - rp * 0.6;
  let fx = nx * 0.5 * slow, fy = ny * 0.5 * slow;
  if (phaseType === "inhale") {
    const str = (0.2 + phaseT * 0.4) * slow;
    fx += ndx * str * (1 + 40 / (dist + 25)); fy += ndy * str * (1 + 40 / (dist + 25));
    p.opacity = lerpVal(p.opacity, (0.5 + phaseT * 0.15) * (1 - rp * 0.3), dt * 2);
  } else if (phaseType === "hold") { fx *= 0.2; fy *= 0.2; p.opacity = lerpVal(p.opacity, 0.55 * (1 - rp * 0.3), dt * 1.5);
  } else {
    const str = (0.15 + phaseT * 0.3) * slow;
    fx -= ndx * str * (1 + 20 / (dist + 30)); fy -= ndy * str * (1 + 20 / (dist + 30));
    p.opacity = lerpVal(p.opacity, (0.4 - phaseT * 0.1) * (1 - rp * 0.4), dt * 2);
  }
  return { fx, fy };
}

// --- NEW MINDFULNESS MOTIONS ---

function motionRiver(p: Particle, _n: number, _n2: number, _d: number,
  _pt: string, _phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  // Gentle left-to-right river current
  let fx = 0.4 + nx * 0.2;
  let fy = ny * 0.15 + Math.sin(t * 0.6 + p.noiseOffsetX * 2) * 0.15;
  // Slight vertical undulation like water
  fy += Math.sin(p.x / WIDTH * Math.PI * 3 + t * 0.5) * 0.12;
  p.opacity = lerpVal(p.opacity, 0.4 + Math.sin(t * 0.8 + p.noiseOffsetY) * 0.1, dt * 1.5);
  // Wrap right to left
  if (p.x > WIDTH + 30) { p.x = -20; p.y = HEIGHT * 0.3 + Math.random() * HEIGHT * 0.4; }
  return { fx, fy };
}

function motionWarmPulse(p: Particle, ndx: number, ndy: number, dist: number,
  _pt: string, _phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  // Slow warm pulsing — converge and expand gently on a slow sine wave
  const pulse = Math.sin(t * 0.4) * 0.5 + 0.5; // 0 to 1
  let fx = nx * 0.3, fy = ny * 0.3;
  // Gentle pull toward center on pulse peak, release on trough
  const str = (pulse - 0.5) * 0.3;
  fx += ndx * str * (1 + 20 / (dist + 30));
  fy += ndy * str * (1 + 20 / (dist + 30));
  p.opacity = lerpVal(p.opacity, 0.35 + pulse * 0.25, dt * 1.5);
  p.size = lerpVal(p.size, p.baseSize * (0.9 + pulse * 0.3), dt * 2);
  return { fx, fy };
}

function motionOrbit(p: Particle, _n: number, _n2: number, dist: number,
  _pt: string, _phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  const cx = WIDTH / 2, cy = HEIGHT / 2;
  const angle = Math.atan2(p.y - cy, p.x - cx);
  // Slow orbital motion around center
  const orbitSpeed = 0.15 / (1 + dist * 0.01);
  let fx = Math.cos(angle + Math.PI / 2) * orbitSpeed + nx * 0.15;
  let fy = Math.sin(angle + Math.PI / 2) * orbitSpeed + ny * 0.15;
  // Gentle pull toward a ring at ~80px from center
  const targetDist = 80;
  const radialForce = (targetDist - dist) * 0.002;
  fx += (p.x < cx ? -1 : 1) * Math.abs(p.x - cx) / dist * radialForce;
  fy += (p.y < cy ? -1 : 1) * Math.abs(p.y - cy) / dist * radialForce;
  p.opacity = lerpVal(p.opacity, 0.4 + Math.sin(t * 0.5 + p.noiseOffsetX) * 0.1, dt * 1.5);
  return { fx, fy };
}

function motionSensory(p: Particle, _n: number, _n2: number, _d: number,
  _pt: string, _phaseT: number, t: number, dt: number, nx: number, ny: number, _l: string, _rp: number, promptIdx: number): ForceResult {
  // Each sense has a different feel
  let fx = nx * 0.3, fy = ny * 0.3;
  switch (promptIdx) {
    case 0: // See — light, sparkly, spread out
      p.opacity = lerpVal(p.opacity, 0.5 + Math.sin(t * 4 + p.noiseOffsetX * 5) * 0.2, dt * 3);
      p.size = lerpVal(p.size, p.baseSize * 1.2, dt * 2);
      break;
    case 1: // Touch — slow, textured, close together
      fx += (WIDTH / 2 - p.x) * 0.003; fy += (HEIGHT / 2 - p.y) * 0.003;
      p.opacity = lerpVal(p.opacity, 0.55, dt * 2);
      p.size = lerpVal(p.size, p.baseSize * 1.4, dt * 1.5);
      break;
    case 2: // Hear — wave-like horizontal motion
      fx += Math.sin(t * 1.5 + p.y * 0.05) * 0.3;
      p.opacity = lerpVal(p.opacity, 0.4 + Math.sin(t * 2 + p.noiseOffsetY) * 0.15, dt * 2);
      break;
    case 3: // Smell — drift upward gently
      fy -= 0.2; fx += Math.sin(t * 0.5 + p.noiseOffsetX) * 0.15;
      p.opacity = lerpVal(p.opacity, 0.35, dt * 2);
      if (p.y < -20) { p.y = HEIGHT + 10; }
      break;
    case 4: // Taste — subtle, small, close
      fx += (WIDTH / 2 - p.x) * 0.005; fy += (HEIGHT / 2 - p.y) * 0.005;
      p.opacity = lerpVal(p.opacity, 0.3, dt * 2);
      p.size = lerpVal(p.size, p.baseSize * 0.8, dt * 2);
      break;
  }
  return { fx, fy };
}

function motionAmbient(p: Particle, _n: number, _n2: number, _d: number,
  _pt: string, _phaseT: number, t: number, dt: number, nx: number, ny: number): ForceResult {
  // Pure gentle drift — no directional force, just noise
  const fx = nx * 0.4;
  const fy = ny * 0.4;
  p.opacity = lerpVal(p.opacity, 0.35 + Math.sin(t * 0.6 + p.noiseOffsetX) * 0.1, dt * 1.5);
  p.size = lerpVal(p.size, p.baseSize * (1 + Math.sin(t * 0.3 + p.noiseOffsetY) * 0.15), dt * 1.5);
  return { fx, fy };
}

// =====================================================================
// ANIMATION STATE
// =====================================================================
interface AnimState {
  particles: Particle[];
  noise: ReturnType<typeof createNoise>;
  technique: Technique;
  time: number;
  introTime: number; introDone: boolean;
  // Breathing state
  phaseIndex: number; phaseTime: number; round: number;
  // Mindfulness state
  promptIndex: number; promptTime: number; promptOpacity: number;
  // Shared
  done: boolean; doneTime: number;
  bgColor: [number, number, number]; targetBgColor: [number, number, number];
  labelOpacity: number; nameOpacity: number; subtitleOpacity: number;
}

function createState(): AnimState {
  return {
    particles: Array.from({ length: PARTICLE_COUNT }, createParticle),
    noise: createNoise(),
    technique: randomTechnique(),
    time: 0, introTime: 0, introDone: false,
    phaseIndex: 0, phaseTime: 0, round: 0,
    promptIndex: 0, promptTime: 0, promptOpacity: 0,
    done: false, doneTime: 0,
    bgColor: [260, 15, 11], targetBgColor: [260, 15, 11],
    labelOpacity: 0, nameOpacity: 0, subtitleOpacity: 0,
  };
}

// =====================================================================
// COMPONENT
// =====================================================================
interface Props { onComplete: () => void; }

export default function BreathingSession({ onComplete }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef<AnimState>(createState());
  const rafRef = useRef<number>(0);

  const handleDone = useCallback(async () => { await invoke("hide_panel"); }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = WIDTH * dpr; canvas.height = HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    let lastTime = performance.now();

    function tick(now: number) {
      const rawDt = (now - lastTime) / 1000;
      lastTime = now;

      // If more than 0.5s passed, the panel was hidden and just reopened — reset
      if (rawDt > 0.5) {
        stateRef.current = createState();
      }

      const dt = Math.min(rawDt, 0.05);
      const s = stateRef.current;
      s.time += dt;

      const technique = s.technique;
      const visual = technique.visual;
      const cx = WIDTH / 2, cy = HEIGHT / 2;
      const isMindful = technique.kind === "mindfulness";

      // ── INTRO ──
      if (!s.introDone) {
        s.introTime += dt;
        if (s.introTime < 1) { s.nameOpacity = lerpVal(s.nameOpacity, 0.7, dt * 4); s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0.35, dt * 3); }
        else if (s.introTime < 2) { s.nameOpacity = 0.7; s.subtitleOpacity = 0.35; }
        else { s.nameOpacity = lerpVal(s.nameOpacity, 0, dt * 4); s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0, dt * 4); }
        if (s.introTime >= INTRO_DURATION) {
          s.introDone = true; s.nameOpacity = 0; s.subtitleOpacity = 0;
          if (!isMindful) {
            const bt = technique as BreathingTechnique;
            s.targetBgColor = [...visual.colors[bt.phases[0].type]];
          } else {
            s.targetBgColor = [...visual.colors.hold]; // start neutral
          }
        }
        s.targetBgColor = [260, 15, 11];
      }

      // ── BREATHING logic ──
      let currentPhaseType = "hold";
      let currentLabel = "";
      let phaseT = 0;

      if (s.introDone && !s.done && !isMindful) {
        const bt = technique as BreathingTechnique;
        const phase = bt.phases[s.phaseIndex];
        currentPhaseType = phase.type; currentLabel = phase.label;
        s.phaseTime += dt;
        phaseT = Math.min(s.phaseTime / phase.duration, 1);
        if (s.phaseTime >= phase.duration) {
          s.phaseTime -= phase.duration; s.phaseIndex++;
          if (s.phaseIndex >= bt.phases.length) { s.phaseIndex = 0; s.round++; }
          if (s.round >= bt.rounds) { s.done = true; s.doneTime = 0; }
          else { s.targetBgColor = [...visual.colors[bt.phases[s.phaseIndex].type]]; }
        }
      }

      // ── MINDFULNESS logic ──
      let currentPromptText = "";
      let promptProgress = 0;

      if (s.introDone && !s.done && isMindful) {
        const mt = technique as MindfulnessTechnique;
        const prompt = mt.prompts[s.promptIndex];
        currentPromptText = prompt.text;
        s.promptTime += dt;
        promptProgress = Math.min(s.promptTime / prompt.duration, 1);

        // Fade in first 1.5s, hold, fade out last 1s
        const fadeIn = Math.min(s.promptTime / 1.5, 1);
        const fadeOut = s.promptTime > prompt.duration - 1 ? Math.max(0, (prompt.duration - s.promptTime) / 1) : 1;
        s.promptOpacity = lerpVal(s.promptOpacity, fadeIn * fadeOut * 0.6, dt * 4);

        // Shift background color slowly through the palette
        const colorT = s.promptIndex / mt.prompts.length;
        const fromColor = visual.colors.inhale;
        const toColor = visual.colors.exhale;
        s.targetBgColor = lerpHSL(fromColor, toColor, colorT);

        if (s.promptTime >= prompt.duration) {
          s.promptTime = 0; s.promptIndex++; s.promptOpacity = 0;
          if (s.promptIndex >= mt.prompts.length) { s.done = true; s.doneTime = 0; }
        }
      }

      // ── COMPLETION ──
      if (s.done) {
        s.doneTime += dt;
        if (s.doneTime < 1.5) s.targetBgColor = [35, 30, 14];
        else s.targetBgColor = [30, 15, 10];
        if (s.doneTime > 7) { onComplete(); return; }
      }

      s.bgColor = lerpHSL(s.bgColor, s.targetBgColor, dt * 1.2);
      const roundProgress = !isMindful && (technique as BreathingTechnique).rounds > 0
        ? s.round / (technique as BreathingTechnique).rounds : 0;

      // ── UPDATE PARTICLES ──
      for (const p of s.particles) {
        const t = s.time;
        const nx = s.noise(p.noiseOffsetX + t * 0.3, p.noiseOffsetY) * 0.8;
        const ny = s.noise(p.noiseOffsetX, p.noiseOffsetY + t * 0.3) * 0.8;
        const dx = cx - p.x, dy = cy - p.y;
        const dist = Math.sqrt(dx * dx + dy * dy) + 0.1;
        const ndx = dx / dist, ndy = dy / dist;
        let force: ForceResult;

        if (s.done) {
          const doneT = s.doneTime;
          if (doneT < 1.2) {
            const burst = (1.2 - doneT) / 1.2;
            force = { fx: -ndx * 2.5 * burst + nx * 0.5, fy: -ndy * 2.5 * burst + ny * 0.5 - 0.3 * burst };
            p.opacity = lerpVal(p.opacity, 0.7 + burst * 0.25, dt * 5);
            p.size = lerpVal(p.size, p.baseSize * (1.3 + burst * 0.5), dt * 4);
          } else if (doneT < 3.5) {
            const orbitT = (doneT - 1.2) / 2.3;
            const angle = Math.atan2(p.y - cy, p.x - cx);
            force = { fx: Math.cos(angle + Math.PI / 2) * 0.3 * (1 - orbitT) + nx * 0.2, fy: Math.sin(angle + Math.PI / 2) * 0.3 * (1 - orbitT) + ny * 0.2 };
            p.opacity = lerpVal(p.opacity, 0.5 - orbitT * 0.15, dt * 1.5);
          } else {
            const fadeT = Math.min((doneT - 3.5) / 3, 1);
            p.vx *= 0.92 + 0.07 * fadeT; p.vy *= 0.92 + 0.07 * fadeT;
            p.opacity = lerpVal(p.opacity, 0.15 * (1 - fadeT), dt * 1);
            force = { fx: nx * 0.1, fy: ny * 0.1 };
          }
        } else if (!s.introDone) {
          force = { fx: nx * 0.3, fy: ny * 0.3 }; p.opacity = lerpVal(p.opacity, 0.35, dt * 2);
        } else {
          switch (visual.motion) {
            case "wave": force = motionWave(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "snowfall": force = motionSnowfall(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx); break;
            case "alternate": force = motionAlternate(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny, currentLabel); break;
            case "lunar": force = motionLunar(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "belly": force = motionBelly(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "sedation": force = motionSedation(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny, currentLabel, roundProgress); break;
            case "river": force = motionRiver(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "warm-pulse": force = motionWarmPulse(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "orbit": force = motionOrbit(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            case "sensory": force = motionSensory(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny, "", 0, s.promptIndex); break;
            case "ambient": force = motionAmbient(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny); break;
            default: force = motionConverge(p, ndx, ndy, dist, currentPhaseType, phaseT, t, dt, nx, ny);
          }
        }

        p.vx = p.vx * 0.92 + force.fx * dt * 8;
        p.vy = p.vy * 0.92 + force.fy * dt * 8;
        const maxSpeed = 1.8;
        const spd = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
        if (spd > maxSpeed) { p.vx = (p.vx / spd) * maxSpeed; p.vy = (p.vy / spd) * maxSpeed; }
        p.x += p.vx; p.y += p.vy;
        const margin = 20;
        if (p.x < -margin) p.vx += 0.5; if (p.x > WIDTH + margin) p.vx -= 0.5;
        if (p.y < -margin) p.vy += 0.5; if (p.y > HEIGHT + margin) p.vy -= 0.5;
      }

      // ── RENDER ──
      const [h, sat, l] = s.bgColor;
      ctx.fillStyle = `hsl(${h}, ${sat}%, ${l}%)`; ctx.fillRect(0, 0, WIDTH, HEIGHT);

      const boost = visual.brightnessBoost || 0;
      for (const p of s.particles) {
        const ph = h + (p.noiseOffsetX % 20 - 10);
        const ps = Math.min(sat + 50, 90); const pl = Math.min(l + 50 + boost * 100, 85);
        const op = Math.min(p.opacity + 0.25, 0.95);
        ctx.beginPath(); ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op})`; ctx.fill();
        ctx.beginPath(); ctx.arc(p.x, p.y, p.size * 4, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${ph}, ${ps}%, ${pl}%, ${op * 0.12})`; ctx.fill();
      }

      // ── TEXT ──
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
        const targetOp = doneT > 1.0 ? 0.65 : 0;
        s.labelOpacity = lerpVal(s.labelOpacity, targetOp, dt * 2.5);
        if (doneT > 5.5) s.labelOpacity = lerpVal(s.labelOpacity, 0, dt * 3);
        ctx.font = "200 20px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 245, 230, ${s.labelOpacity})`;
        ctx.fillText("well done", cx, cy - 8);
        if (doneT > 1.8) {
          const subOp = Math.min((doneT - 1.8) * 0.8, 0.3);
          const subFinal = doneT > 5.5 ? lerpVal(subOp, 0, (doneT - 5.5) * 2) : subOp;
          ctx.font = "300 12px -apple-system, BlinkMacSystemFont, sans-serif";
          ctx.fillStyle = `rgba(255, 245, 230, ${Math.max(0, subFinal)})`;
          ctx.fillText("take this calm with you", cx, cy + 16);
        }
      } else if (isMindful) {
        // Mindfulness prompt text — centered, wrapped, gentle
        ctx.font = "300 17px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 250, 240, ${s.promptOpacity})`;
        const lines = wrapText(ctx, currentPromptText, WIDTH - 60);
        const lineHeight = 24;
        const startY = cy - ((lines.length - 1) * lineHeight) / 2;
        lines.forEach((line, i) => ctx.fillText(line, cx, startY + i * lineHeight));
      } else {
        // Breathing instruction
        s.labelOpacity = lerpVal(s.labelOpacity, 0.5, dt * 3);
        ctx.font = "300 15px -apple-system, BlinkMacSystemFont, sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity})`;
        ctx.fillText(currentLabel, cx, cy);
      }

      // Round dots (breathing only)
      if (s.introDone && !s.done && !isMindful) {
        const bt = technique as BreathingTechnique;
        const dotY = HEIGHT - 24; const dotSpacing = 12;
        const startX = cx - ((bt.rounds - 1) * dotSpacing) / 2;
        for (let i = 0; i < bt.rounds; i++) {
          ctx.beginPath(); ctx.arc(startX + i * dotSpacing, dotY, 3, 0, Math.PI * 2);
          ctx.fillStyle = i < s.round ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.5)`
            : i === s.round ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.35)` : `rgba(255, 255, 255, 0.1)`;
          ctx.fill();
        }
      }

      // Progress dots for mindfulness (one per prompt)
      if (s.introDone && !s.done && isMindful) {
        const mt = technique as MindfulnessTechnique;
        const dotY = HEIGHT - 24; const dotSpacing = 12;
        const startX = cx - ((mt.prompts.length - 1) * dotSpacing) / 2;
        for (let i = 0; i < mt.prompts.length; i++) {
          ctx.beginPath(); ctx.arc(startX + i * dotSpacing, dotY, 3, 0, Math.PI * 2);
          ctx.fillStyle = i < s.promptIndex ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.5)`
            : i === s.promptIndex ? `hsla(${h + 40}, ${sat + 20}%, ${l + 25}%, 0.35)` : `rgba(255, 255, 255, 0.1)`;
          ctx.fill();
        }
      }

      rafRef.current = requestAnimationFrame(tick);
    }
    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [onComplete]);

  return (
    <div style={{ width: WIDTH, height: HEIGHT, borderRadius: 12, overflow: "hidden", position: "relative", cursor: "default", userSelect: "none" }}>
      <canvas ref={canvasRef} style={{ width: WIDTH, height: HEIGHT, display: "block" }} />
      <button onClick={handleDone} title="Close" style={{
        position: "absolute", top: 8, right: 8, width: 26, height: 26,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: "transparent", border: "none", color: "rgba(255,255,255,0.2)",
        cursor: "pointer", borderRadius: 6, zIndex: 10, transition: "all 0.2s ease", outline: "none",
      }}
        onMouseEnter={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.5)"; e.currentTarget.style.background = "rgba(255,255,255,0.08)"; }}
        onMouseLeave={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.2)"; e.currentTarget.style.background = "transparent"; }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"><path d="M2 2l8 8M10 2l-8 8" /></svg>
      </button>
    </div>
  );
}
