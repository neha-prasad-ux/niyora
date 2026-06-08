import { useRef, useEffect, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  randomTechnique,
  breathingList,
  mindfulnessList,
  type Technique,
  type BreathingTechnique,
  type MindfulnessTechnique,
} from "./techniques";
import { TIERS, currentTier } from "./tiers";

function intendedDurationSec(t: Technique): number {
  if (t.kind === "breathing") {
    const perRound = t.phases.reduce((sum, p) => sum + p.duration, 0);
    return perRound * t.rounds;
  }
  return t.prompts.reduce((sum, p) => sum + p.duration, 0);
}
import { scoreToBallGradient, scoreToOrbAccent, type SituationalSnapshot } from "./useSnapshot";

// --- Config ---
const LG_WIDTH = 460;
const LG_HEIGHT = 640;
// Used by motion functions outside the component
let WIDTH = LG_WIDTH;
let HEIGHT = LG_HEIGHT;
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

function createParticle(w: number, h: number): Particle {
  const angle = Math.random() * Math.PI * 2;
  const dist = 30 + Math.random() * 120;
  const x = w / 2 + Math.cos(angle) * dist;
  const y = h / 2 + Math.sin(angle) * dist;
  return {
    x, y, vx: 0, vy: 0,
    baseSize: 1.5 + Math.random() * 1.5, size: 1.5 + Math.random() * 1.5,
    opacity: 0.3 + Math.random() * 0.4,
    noiseOffsetX: Math.random() * 1000, noiseOffsetY: Math.random() * 1000,
    homeX: x, homeY: y, side: x < w / 2 ? -1 : 1,
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
  const riverY = HEIGHT * 0.5;
  const riverBand = 80;
  const isWater = p.noiseOffsetX % 1 < 0.6; // 60% water, 40% air

  if (isWater) {
    // Water particles: flow rightward within the river band
    let fx = 0.5 + nx * 0.15;
    let fy = ny * 0.1;
    // Undulation with the water lines
    fy += Math.sin(p.x / WIDTH * Math.PI * 2.5 + t * 0.7 + p.noiseOffsetY) * 0.12;
    // Pull toward river band vertically
    const distFromRiver = p.y - riverY;
    fy -= distFromRiver * 0.008;
    p.opacity = lerpVal(p.opacity, 0.35 + Math.sin(t * 0.8 + p.noiseOffsetY) * 0.1, dt * 1.5);
    p.size = lerpVal(p.size, p.baseSize * 0.9, dt * 2);
    // Wrap right to left within river band
    if (p.x > WIDTH + 20) {
      p.x = -15;
      p.y = riverY + (Math.random() - 0.5) * riverBand * 0.8;
    }
    return { fx, fy };
  } else {
    // Air particles: gentle ambient drift above and below the river
    let fx = nx * 0.25 + 0.05; // very slight rightward drift
    let fy = ny * 0.25;
    // Push away from river center to create clear separation
    const distFromRiver = p.y - riverY;
    if (Math.abs(distFromRiver) < riverBand * 0.6) {
      fy += Math.sign(distFromRiver) * 0.1;
    }
    p.opacity = lerpVal(p.opacity, 0.2 + Math.sin(t * 0.5 + p.noiseOffsetX) * 0.05, dt * 1);
    p.size = lerpVal(p.size, p.baseSize * 0.7, dt * 2);
    if (p.x > WIDTH + 30) { p.x = -20; }
    return { fx, fy };
  }
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
  waiting: boolean; // true = pre-session info screen, particles drift but no intro
  paused: boolean; // user tapped the canvas to pause; freezes phase/prompt timers
  introTime: number; introDone: boolean;
  // Breathing state
  phaseIndex: number; phaseTime: number; round: number;
  // Mindfulness state
  promptIndex: number; promptTime: number; promptOpacity: number;
  // River/leaf state (for "Leaves on a Stream")
  leafX: number; leafY: number; leafOpacity: number; leafVisible: boolean;
  // Shared
  done: boolean; doneTime: number;
  bgColor: [number, number, number]; targetBgColor: [number, number, number];
  labelOpacity: number; nameOpacity: number; subtitleOpacity: number;
  // Cross-fade between phase labels (Inhale -> Hold -> Exhale...).
  // labelT advances 0 -> 1 across LABEL_FADE_MS; while < 1, the previous
  // label is drawn at (1 - labelT) and the current at labelT, so swaps
  // never read as a hard cut.
  shownLabel: string; prevLabel: string; labelT: number;
  shownPromptIndex: number;
}

function createState(w: number, h: number, completedSessions?: number, forceBoxBreath?: boolean): AnimState {
  const technique = forceBoxBreath
    ? (breathingList.find((t) => t.name === "Box Breath") ?? randomTechnique(completedSessions))
    : randomTechnique(completedSessions);
  return {
    particles: Array.from({ length: PARTICLE_COUNT }, () => createParticle(w, h)),
    noise: createNoise(),
    technique,
    time: 0, waiting: true, paused: false, introTime: 0, introDone: false,
    phaseIndex: 0, phaseTime: 0, round: 0,
    promptIndex: 0, promptTime: 0, promptOpacity: 0,
    leafX: w * 0.35, leafY: h * 0.5, leafOpacity: 0, leafVisible: false,
    done: false, doneTime: 0,
    bgColor: [260, 15, 11], targetBgColor: [260, 15, 11],
    labelOpacity: 0, nameOpacity: 0, subtitleOpacity: 0,
    shownLabel: "", prevLabel: "", labelT: 1,
    shownPromptIndex: -1,
  };
}

// =====================================================================
// AUDIO
// =====================================================================
const AMBIENT_TRACKS: Record<string, string> = {
  serene: "/audio/serene.mp3",
  ocean: "/audio/ocean.mp3",
  forest: "/audio/forest.wav",
};
const TRACK_KEYS = Object.keys(AMBIENT_TRACKS) as Array<keyof typeof AMBIENT_TRACKS>;

function resolveTrack(pref: string): string {
  if (pref === "random" || !AMBIENT_TRACKS[pref]) {
    return AMBIENT_TRACKS[TRACK_KEYS[Math.floor(Math.random() * TRACK_KEYS.length)]];
  }
  return AMBIENT_TRACKS[pref];
}

// =====================================================================
// BUTTON PARTICLES
// =====================================================================
interface BtnParticle {
  id: number;
  x: number; y: number;
  vx: number; vy: number;
  size: number;
  opacity: number;
  hue: number;
}

function useButtonParticles() {
  const [particles, setParticles] = useState<BtnParticle[]>([]);
  const idRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const startEmitting = useCallback(() => {
    if (intervalRef.current) return;
    intervalRef.current = setInterval(() => {
      const angle = Math.random() * Math.PI * 2;
      const speed = 1.5 + Math.random() * 2.5;
      const p: BtnParticle = {
        id: idRef.current++,
        x: 0, y: 0,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        size: 3 + Math.random() * 4,
        opacity: 0.6 + Math.random() * 0.3,
        hue: 260 + Math.random() * 40,
      };
      setParticles(prev => [...prev.slice(-20), p]);
    }, 60);
  }, []);

  const stopEmitting = useCallback(() => {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }, []);

  // Animate particles outward + fade
  useEffect(() => {
    if (particles.length === 0) return;
    const frame = requestAnimationFrame(() => {
      setParticles(prev => prev
        .map(p => ({
          ...p,
          x: p.x + p.vx,
          y: p.y + p.vy,
          vx: p.vx * 0.97,
          vy: p.vy * 0.97,
          opacity: p.opacity - 0.02,
        }))
        .filter(p => p.opacity > 0)
      );
    });
    return () => cancelAnimationFrame(frame);
  }, [particles]);

  return { particles, startEmitting, stopEmitting };
}

// =====================================================================
// COMPONENT
// =====================================================================
interface Props {
  /** Stable id for this session, minted by App.tsx so the pre-session
   *  "Measure stress" tap, the record_session call, and the post-mood
   *  "Measure stress" tap all bind to the same row. */
  sessionId: string;
  onComplete: () => void;
  snapshot?: SituationalSnapshot | null;
  completedSessions?: number;
  /** True on the day's first panel open. Forces Box Breath and swaps the
   *  intro copy for the "start your day" framing. */
  isFirstOpenToday?: boolean;
}

export default function BreathingSession({ sessionId, onComplete, snapshot, completedSessions, isFirstOpenToday }: Props) {
  WIDTH = LG_WIDTH;
  HEIGHT = LG_HEIGHT;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef<AnimState>(createState(LG_WIDTH, LG_HEIGHT, completedSessions, isFirstOpenToday));
  const rafRef = useRef<number>(0);
  const sizeRef = useRef({ w: LG_WIDTH, h: LG_HEIGHT });
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const reduceMotionRef = useRef(false);
  const trackMenuRef = useRef<HTMLDivElement | null>(null);
  const [muted, setMuted] = useState(false);
  const [preferredTrack, setPreferredTrack] = useState("random");
  const [showTrackMenu, setShowTrackMenu] = useState(false);
  const [waiting, setWaiting] = useState(true);
  const [transitioning, setTransitioning] = useState(false);
  const [paused, setPaused] = useState(false);
  const [showPicker, setShowPicker] = useState(false);
  // Forces a re-render when the user picks a different technique while the
  // info screen is showing — stateRef.current.technique mutates, but React
  // needs a nudge to redraw the technique benefits / name.
  const [, setPickNonce] = useState(0);
  const [announcedPhase, setAnnouncedPhase] = useState('');
  // Once the user picks a technique from "Try a different one", drop the
  // day-start framing and show the real technique name/subtitle.
  const [showDayStart, setShowDayStart] = useState(!!isFirstOpenToday);
  // Companion state-machine card replaces the earlier secondary button.
  // The card handles its own paired/measuring/result lifecycle so this
  // view doesn't need to track those individual flags anymore.

  // Compute which techniques are locked at the user's current tier.
  const userTier = currentTier(completedSessions ?? 0);
  const userTierIdx = TIERS.findIndex((t) => t.id === userTier.id);
  const isLocked = (t: Technique) => {
    const techIdx = TIERS.findIndex((x) => x.id === t.unlockTier);
    return techIdx > userTierIdx;
  };
  const unlockTierName = (t: Technique) =>
    TIERS.find((x) => x.id === t.unlockTier)?.name ?? "";
  const lockTooltip = (t: Technique) => {
    const need = TIERS.find((x) => x.id === t.unlockTier)?.threshold ?? 0;
    const remaining = Math.max(0, need - (completedSessions ?? 0));
    if (remaining <= 0) return `Practice more to unlock`;
    return `Opens after ${remaining} more ${remaining === 1 ? "practice" : "practices"} · ${unlockTierName(t)}`;
  };

  const selectTechnique = useCallback((t: Technique) => {
    stateRef.current.technique = t;
    // Reset intro/animation state so the new technique gets a fresh start.
    stateRef.current.introTime = 0;
    stateRef.current.introDone = false;
    stateRef.current.phaseIndex = 0;
    stateRef.current.phaseTime = 0;
    stateRef.current.round = 0;
    stateRef.current.promptIndex = 0;
    stateRef.current.promptTime = 0;
    setShowPicker(false);
    setShowDayStart(false);
    setPickNonce((n) => n + 1);
  }, []);
  const btnParticles = useButtonParticles();

  // Session-recording refs. Session is "really started" once the 3s intro
  // finishes. Closes during the intro are not recorded (user didn't commit).
  const sessionInfoRef = useRef<{
    startedAt: string;
    startTimeMs: number;
    techniqueName: string;
    techniqueKind: string;
    intendedDurationSec: number;
  } | null>(null);
  const loggedRef = useRef(false);

  const logSession = useCallback((completed: boolean) => {
    const info = sessionInfoRef.current;
    if (!info || loggedRef.current) return;
    loggedRef.current = true;
    const actualSec = Math.max(0, Math.round((performance.now() - info.startTimeMs) / 1000));
    invoke("record_session", {
      sessionId,
      startedAt: info.startedAt,
      techniqueName: info.techniqueName,
      techniqueKind: info.techniqueKind,
      intendedDurationSec: info.intendedDurationSec,
      actualDurationSec: actualSec,
      completed,
    }).catch(() => {
      /* best-effort; never block UX on storage */
    });
  }, [sessionId]);

  const handleBegin = useCallback(() => {
    btnParticles.stopEmitting();
    setTransitioning(true);
    // Tell Rust the session is active so click-outside-to-dismiss is
    // suppressed until the session ends.
    invoke("set_session_active", { active: true }).catch(() => {});
    // After animation completes, start the session
    setTimeout(() => {
      stateRef.current.waiting = false;
      setWaiting(false);
      setTransitioning(false);
    }, 1300);
  }, []);

  const toggleMute = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (audio.muted) { audio.muted = false; setMuted(false); }
    else { audio.muted = true; setMuted(true); }
  }, []);

  const togglePause = useCallback(() => {
    const s = stateRef.current;
    // Only allow pause during the active practice — not on the info screen,
    // not during the 3s intro, not on the "well done" outro.
    if (!s.introDone || s.done || s.waiting) return;
    s.paused = !s.paused;
    setPaused(s.paused);
    const audio = audioRef.current;
    if (audio) {
      if (s.paused) {
        audio.pause();
      } else if (!muted) {
        audio.play().catch(() => {});
      }
    }
  }, [muted]);

  const handleDone = useCallback(async () => {
    // Record an abandoned session if the user got past the intro but didn't
    // finish. If they're already in the "done" state, the completed log fired
    // when rounds finished; loggedRef prevents double-recording either way.
    if (sessionInfoRef.current && !loggedRef.current) {
      logSession(false);
    }
    // Session is over — re-enable click-outside-to-dismiss.
    invoke("set_session_active", { active: false }).catch(() => {});
    // Fade out audio before closing
    const audio = audioRef.current;
    if (audio) {
      let v = audio.volume;
      const fade = setInterval(() => {
        v = Math.max(v - 0.05, 0);
        audio.volume = v;
        if (v <= 0) { clearInterval(fade); audio.pause(); audio.currentTime = 0; }
      }, 30);
    }
    await invoke("hide_panel");
  }, [logSession]);

  // Belt-and-suspenders: on unmount (e.g. session auto-advances to mood),
  // make sure Rust knows the session is no longer active.
  useEffect(() => {
    return () => {
      invoke("set_session_active", { active: false }).catch(() => {});
    };
  }, []);

  // Load persisted track preference on mount.
  useEffect(() => {
    invoke<string>("get_preferred_track").then(setPreferredTrack).catch(() => {});
  }, []);

  // Read once at mount; stored in a ref so the draw loop can check it
  // per frame without triggering re-renders.
  useEffect(() => {
    reduceMotionRef.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }, []);

  // Close the track dropdown when the user clicks anywhere outside it.
  // Same dismiss semantics as every other popover-style UI in the app.
  useEffect(() => {
    if (!showTrackMenu) return;
    const onPointerDown = (e: PointerEvent) => {
      const root = trackMenuRef.current;
      if (root && !root.contains(e.target as Node)) {
        setShowTrackMenu(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [showTrackMenu]);

  // Background audio — starts only after the user clicks Begin (so the panel
  // can pop up silently and the user controls when the soundscape kicks in).
  // Fades in on start, fades out on unmount.
  useEffect(() => {
    if (waiting) return;
    const audio = new Audio(resolveTrack(preferredTrack));
    audio.loop = true;
    audio.volume = 0;
    audioRef.current = audio;
    audio.play().catch(() => {}); // autoplay may be blocked

    let vol = 0;
    const fadeIn = setInterval(() => {
      vol = Math.min(vol + 0.02, 0.35);
      audio.volume = vol;
      if (vol >= 0.35) clearInterval(fadeIn);
    }, 80);

    return () => {
      clearInterval(fadeIn);
      let v = audio.volume;
      const fadeOut = setInterval(() => {
        v = Math.max(v - 0.03, 0);
        audio.volume = v;
        if (v <= 0) { clearInterval(fadeOut); audio.pause(); }
      }, 50);
    };
  }, [waiting, preferredTrack]);


  useEffect(() => {
    sizeRef.current = { w: WIDTH, h: HEIGHT };
  }, [WIDTH, HEIGHT]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = WIDTH * dpr; canvas.height = HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    // Reinitialize particles for new size, but keep the same technique + progress
    const prev = stateRef.current;
    const fresh = createState(WIDTH, HEIGHT);
    fresh.technique = prev.technique;
    fresh.waiting = prev.waiting;
    fresh.introDone = prev.introDone;
    fresh.introTime = prev.introTime;
    fresh.phaseIndex = prev.phaseIndex;
    fresh.phaseTime = prev.phaseTime;
    fresh.round = prev.round;
    fresh.promptIndex = prev.promptIndex;
    fresh.promptTime = prev.promptTime;
    fresh.done = prev.done;
    fresh.doneTime = prev.doneTime;
    fresh.bgColor = prev.bgColor;
    fresh.targetBgColor = prev.targetBgColor;
    fresh.shownLabel = prev.shownLabel;
    fresh.prevLabel = prev.prevLabel;
    fresh.labelT = prev.labelT;
    fresh.shownPromptIndex = prev.shownPromptIndex;
    stateRef.current = fresh;

    let lastTime = performance.now();

    function tick(now: number) {
      const { w: WIDTH, h: HEIGHT } = sizeRef.current;
      const rawDt = (now - lastTime) / 1000;
      lastTime = now;

      // Clamp the per-frame delta so a stalled frame (background throttling,
      // GC pause, paused tab) doesn't fast-forward the session. Component
      // remount on tray reopen handles the "fresh start" case via openKey,
      // so we no longer reset state here — that previously re-rolled the
      // technique mid-session and corrupted session-log metadata.
      const dt = Math.min(rawDt, 0.05);
      const s = stateRef.current;
      s.time += dt;

      const technique = s.technique;
      const visual = technique.visual;
      const cx = WIDTH / 2, cy = HEIGHT / 2;
      const isMindful = technique.kind === "mindfulness";

      // ── WAITING (pre-session info screen) ──
      // Particles drift but nothing else progresses
      if (s.waiting) {
        s.targetBgColor = [260, 15, 11];
      }

      // ── INTRO ──
      if (!s.waiting && !s.introDone) {
        if (reduceMotionRef.current) s.introTime = INTRO_DURATION;
        s.introTime += dt;
        if (s.introTime < 1) { s.nameOpacity = lerpVal(s.nameOpacity, 0.9, dt * 4); s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0.55, dt * 3); }
        else if (s.introTime < 2) { s.nameOpacity = 0.9; s.subtitleOpacity = 0.55; }
        else { s.nameOpacity = lerpVal(s.nameOpacity, 0, dt * 4); s.subtitleOpacity = lerpVal(s.subtitleOpacity, 0, dt * 4); }
        if (s.introTime >= INTRO_DURATION) {
          s.introDone = true; s.nameOpacity = 0; s.subtitleOpacity = 0;
          // Mark the session as truly started — past the intro the user has committed.
          // Closes before this point are not recorded.
          if (!sessionInfoRef.current) {
            sessionInfoRef.current = {
              startedAt: new Date().toISOString(),
              startTimeMs: performance.now(),
              techniqueName: technique.name,
              techniqueKind: technique.kind,
              intendedDurationSec: intendedDurationSec(technique),
            };
          }
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

      if (s.introDone && !s.done && !isMindful && !s.paused) {
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
      // prompt progress tracking

      if (s.introDone && !s.done && isMindful && !s.paused) {
        const mt = technique as MindfulnessTechnique;
        const prompt = mt.prompts[s.promptIndex];
        currentPromptText = prompt.text;
        if (s.promptIndex !== s.shownPromptIndex) {
          s.shownPromptIndex = s.promptIndex;
          setAnnouncedPhase(currentPromptText);
        }
        s.promptTime += dt;
        // promptProgress used implicitly via s.promptTime / prompt.duration

        // Fade in first 1.5s, hold, fade out last 1s
        const fadeIn = Math.min(s.promptTime / 1.5, 1);
        const fadeOut = s.promptTime > prompt.duration - 1 ? Math.max(0, (prompt.duration - s.promptTime) / 1) : 1;
        s.promptOpacity = lerpVal(s.promptOpacity, fadeIn * fadeOut * 0.85, dt * 4);

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
        // Record the completed session on the first frame after rounds finish.
        // loggedRef guards against double-recording if user closes during "well done".
        if (!loggedRef.current && sessionInfoRef.current) {
          logSession(true);
        }
        s.doneTime += dt;
        if (s.doneTime < 1.5) s.targetBgColor = [35, 30, 14];
        else s.targetBgColor = [30, 15, 10];
        // Fade audio out during "well done" screen
        const audio = audioRef.current;
        if (audio && !audio.muted && audio.volume > 0) {
          audio.volume = Math.max(0, audio.volume - dt * 0.15);
          if (audio.volume <= 0) { audio.pause(); audio.currentTime = 0; }
        }
        if (s.doneTime > 4) { onComplete(); return; }
      }

      s.bgColor = lerpHSL(s.bgColor, s.targetBgColor, dt * 1.2);
      const roundProgress = !isMindful && (technique as BreathingTechnique).rounds > 0
        ? s.round / (technique as BreathingTechnique).rounds : 0;

      // ── UPDATE PARTICLES ──
      // When paused, skip physics so the "video" freezes on the current frame.
      // Audio is paused separately in togglePause().
      if (!s.paused && !reduceMotionRef.current) for (const p of s.particles) {
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
      // Background: vertical gradient — colored at top, dark at bottom
      const bgGrad = ctx.createLinearGradient(0, 0, 0, HEIGHT);
      bgGrad.addColorStop(0, `hsl(${h}, ${Math.min(sat + 15, 45)}%, ${l + 6}%)`);
      bgGrad.addColorStop(0.5, `hsl(${h}, ${sat}%, ${l}%)`);
      bgGrad.addColorStop(1, `hsl(${h}, ${Math.max(sat - 5, 5)}%, ${Math.max(l - 3, 5)}%)`);
      ctx.fillStyle = bgGrad; ctx.fillRect(0, 0, WIDTH, HEIGHT);
      // Center radial glow overlay
      const glowGrad = ctx.createRadialGradient(cx, cy * 0.7, 0, cx, cy * 0.7, HEIGHT * 0.5);
      glowGrad.addColorStop(0, `hsla(${h}, ${Math.min(sat + 30, 60)}%, ${l + 10}%, 0.2)`);
      glowGrad.addColorStop(0.6, `hsla(${h}, ${Math.min(sat + 15, 50)}%, ${l + 4}%, 0.06)`);
      glowGrad.addColorStop(1, `hsla(${h}, ${sat}%, ${l}%, 0)`);
      ctx.fillStyle = glowGrad; ctx.fillRect(0, 0, WIDTH, HEIGHT);

      // ── RIVER + LEAF (only for "river" motion) ──
      if (visual.motion === "river" && s.introDone && !s.done && !reduceMotionRef.current) {
        const riverY = HEIGHT * 0.5; // river center line
        const riverBand = 80; // river height band

        // Draw flowing water lines
        ctx.save();
        for (let i = 0; i < 6; i++) {
          const lineY = riverY - riverBand / 2 + (riverBand / 5) * i;
          const lineOpacity = 0.06 + (i === 2 || i === 3 ? 0.03 : 0);
          ctx.beginPath();
          ctx.moveTo(-10, lineY);
          for (let x = 0; x <= WIDTH + 10; x += 5) {
            const wave = Math.sin((x / WIDTH) * Math.PI * 2.5 + s.time * 0.7 + i * 0.8) * 6;
            const wave2 = Math.sin((x / WIDTH) * Math.PI * 4 + s.time * 0.4 + i * 1.2) * 3;
            ctx.lineTo(x, lineY + wave + wave2);
          }
          ctx.strokeStyle = `hsla(${h + 10}, ${sat + 15}%, ${l + 20}%, ${lineOpacity})`;
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }
        ctx.restore();

        // Update leaf position based on prompt
        const pi = s.promptIndex;
        if (pi >= 1 && pi <= 3) {
          s.leafVisible = true;
          if (pi === 1) {
            // "place the thought on a leaf" — leaf fades in at center-left
            s.leafOpacity = lerpVal(s.leafOpacity, 0.8, dt * 2);
            s.leafX = lerpVal(s.leafX, WIDTH * 0.35, dt * 0.5);
          } else if (pi === 2) {
            // "watch it float gently downstream" — leaf drifts right
            s.leafOpacity = lerpVal(s.leafOpacity, 0.75, dt * 1);
            s.leafX += dt * 18; // steady rightward drift
          } else if (pi === 3) {
            // "let the stream carry it away" — leaf at far right, fading
            s.leafX += dt * 25;
            s.leafOpacity = lerpVal(s.leafOpacity, 0, dt * 1.5);
          }
          // Gentle bobbing on the water
          s.leafY = riverY + Math.sin(s.time * 1.2 + 1) * 5;
        } else {
          s.leafVisible = false;
          s.leafOpacity = lerpVal(s.leafOpacity, 0, dt * 3);
          // Reset position for next time
          if (pi === 0) { s.leafX = WIDTH * 0.35; }
        }

        // Draw the leaf
        if (s.leafOpacity > 0.01) {
          ctx.save();
          ctx.translate(s.leafX, s.leafY);
          // Slight rotation from bobbing
          ctx.rotate(Math.sin(s.time * 0.8) * 0.15);
          ctx.globalAlpha = s.leafOpacity;

          // Leaf shape — two bezier curves
          const lw = 14, lh = 8;
          ctx.beginPath();
          ctx.moveTo(-lw / 2, 0);
          ctx.quadraticCurveTo(0, -lh, lw / 2, 0);
          ctx.quadraticCurveTo(0, lh, -lw / 2, 0);
          ctx.closePath();
          ctx.fillStyle = `hsla(90, 35%, 35%, 1)`;
          ctx.fill();

          // Leaf vein
          ctx.beginPath();
          ctx.moveTo(-lw / 2 + 2, 0);
          ctx.lineTo(lw / 2 - 2, 0);
          ctx.strokeStyle = `hsla(90, 25%, 28%, 0.6)`;
          ctx.lineWidth = 0.7;
          ctx.stroke();

          // Soft glow around leaf
          ctx.beginPath();
          ctx.arc(0, 0, 16, 0, Math.PI * 2);
          ctx.fillStyle = `hsla(80, 30%, 40%, ${s.leafOpacity * 0.1})`;
          ctx.fill();

          ctx.globalAlpha = 1;
          ctx.restore();
        }
      }

      // ── TRATAKA FOCAL POINT (only for "orbit" motion) ──
      if (visual.motion === "orbit" && s.introDone && !reduceMotionRef.current) {
        const focusX = cx, focusY = cy;
        // Gentle breathing glow — very slow pulse
        const pulse = 0.5 + Math.sin(s.time * 0.6) * 0.15;
        const glowOpacity = s.done ? lerpVal(0.5, 0, Math.min(s.doneTime / 3, 1)) : pulse;

        // Outer halo (large, soft)
        const grad3 = ctx.createRadialGradient(focusX, focusY, 0, focusX, focusY, 130);
        grad3.addColorStop(0, `hsla(40, 50%, 55%, ${glowOpacity * 0.12})`);
        grad3.addColorStop(1, `hsla(40, 50%, 55%, 0)`);
        ctx.fillStyle = grad3; ctx.fillRect(focusX - 130, focusY - 130, 260, 260);

        // Middle glow
        const grad2 = ctx.createRadialGradient(focusX, focusY, 0, focusX, focusY, 64);
        grad2.addColorStop(0, `hsla(38, 55%, 60%, ${glowOpacity * 0.35})`);
        grad2.addColorStop(1, `hsla(38, 55%, 60%, 0)`);
        ctx.fillStyle = grad2; ctx.fillRect(focusX - 64, focusY - 64, 128, 128);

        // Core dot — warm golden, steady
        ctx.beginPath();
        ctx.arc(focusX, focusY, 18 + pulse * 5, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(38, 60%, 65%, ${glowOpacity * 0.9})`;
        ctx.fill();

        // Inner bright point
        ctx.beginPath();
        ctx.arc(focusX, focusY, 7, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(40, 40%, 85%, ${glowOpacity * 0.85})`;
        ctx.fill();
      }

      const boost = visual.brightnessBoost || 0;
      if (!reduceMotionRef.current) for (const p of s.particles) {
        const ph = h + (p.noiseOffsetX % 20 - 10);
        const ps = Math.min(sat + 50, 90);
        const pl = Math.min(l + 50 + boost * 100, 85);
        const op = Math.min(p.opacity + 0.1, 0.75);
        const r = p.size;

        // Outer glow aura
        const auraR = r * 5;
        const aura = ctx.createRadialGradient(p.x, p.y, r * 0.5, p.x, p.y, auraR);
        aura.addColorStop(0, `hsla(${ph}, ${ps}%, ${pl}%, ${op * 0.15})`);
        aura.addColorStop(1, `hsla(${ph}, ${ps}%, ${pl}%, 0)`);
        ctx.fillStyle = aura;
        ctx.fillRect(p.x - auraR, p.y - auraR, auraR * 2, auraR * 2);

        // Sphere body — radial gradient with highlight
        const hlX = p.x - r * 0.3; // highlight offset top-left
        const hlY = p.y - r * 0.3;
        const sphere = ctx.createRadialGradient(hlX, hlY, r * 0.1, p.x, p.y, r);
        sphere.addColorStop(0, `hsla(${ph}, ${Math.max(ps - 15, 20)}%, ${Math.min(pl + 25, 95)}%, ${op})`); // bright highlight
        sphere.addColorStop(0.4, `hsla(${ph}, ${ps}%, ${pl}%, ${op})`); // mid body
        sphere.addColorStop(0.8, `hsla(${ph}, ${ps + 5}%, ${Math.max(pl - 10, 15)}%, ${op})`); // darker edge
        sphere.addColorStop(1, `hsla(${ph}, ${ps + 10}%, ${Math.max(pl - 20, 10)}%, ${op * 0.6})`); // shadow edge
        ctx.beginPath(); ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
        ctx.fillStyle = sphere; ctx.fill();
      }

      // ── TEXT ──
      ctx.textAlign = "center"; ctx.textBaseline = "middle";
      ctx.letterSpacing = "1.5px";

      if (!s.introDone) {
        ctx.font = "300 22px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.nameOpacity})`;
        ctx.fillText(technique.name, cx, HEIGHT * 0.68);
        ctx.font = "300 13px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.subtitleOpacity})`;
        ctx.fillText(technique.subtitle, cx, HEIGHT * 0.68 + 24);
      } else if (s.done) {
        const doneT = s.doneTime;
        const targetOp = doneT > 1.0 ? 0.85 : 0;
        s.labelOpacity = lerpVal(s.labelOpacity, targetOp, dt * 2.5);
        if (doneT > 5.5) s.labelOpacity = lerpVal(s.labelOpacity, 0, dt * 3);
        ctx.font = "300 20px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 245, 230, ${s.labelOpacity})`;
        ctx.fillText("well done", cx, HEIGHT * 0.70);
        if (doneT > 1.8) {
          const subOp = Math.min((doneT - 1.8) * 0.9, 0.75);
          const subFinal = doneT > 5.5 ? lerpVal(subOp, 0, (doneT - 5.5) * 2) : subOp;
          ctx.font = "300 13px 'Poppins', sans-serif";
          ctx.fillStyle = `rgba(255, 245, 230, ${Math.max(0, subFinal)})`;
          ctx.fillText("take this calm with you", cx, HEIGHT * 0.70 + 24);
          ctx.font = "300 12px 'Poppins', sans-serif";
          ctx.fillStyle = `rgba(255, 245, 230, ${Math.max(0, subFinal * 0.55)})`;
          ctx.fillText(technique.name, cx, HEIGHT * 0.70 + 48);
        }
      } else if (isMindful) {
        // Mindfulness prompt text — lower third, wrapped, gentle
        ctx.font = "500 17px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 250, 240, ${s.promptOpacity})`;
        const lines = wrapText(ctx, currentPromptText, WIDTH - 60);
        const lineHeight = 24;
        const textY = HEIGHT * 0.72;
        const startY = textY - ((lines.length - 1) * lineHeight) / 2;
        lines.forEach((line, i) => ctx.fillText(line, cx, startY + i * lineHeight));
        // Persistent technique instruction below the rotating prompt.
        ctx.font = "400 12px 'Poppins', sans-serif";
        ctx.fillStyle = "rgba(255, 255, 255, 0.7)";
        const instructionLines = wrapText(ctx, technique.instructions, WIDTH - 60);
        instructionLines.forEach((line, i) => {
          ctx.fillText(line, cx, HEIGHT * 0.86 + i * 16);
        });
      } else {
        // Breathing phase label. The current step (Inhale / Hold / Exhale)
        // glows from its own letters via canvas shadowBlur so the active
        // cue reads as "lit up" without any shape sitting behind it. On
        // phase change, the new label cross-fades in over LABEL_FADE_S
        // while the old one fades out.
        s.labelOpacity = lerpVal(s.labelOpacity, 0.9, dt * 3);

        const LABEL_FADE_S = 0.28;
        if (currentLabel !== s.shownLabel) {
          s.prevLabel = s.shownLabel;
          s.shownLabel = currentLabel;
          s.labelT = 0;
          setAnnouncedPhase(currentLabel);
        }
        if (s.labelT < 1) {
          s.labelT = Math.min(1, s.labelT + dt / LABEL_FADE_S);
        }
        // Symmetric ease so neither side dominates the swap moment.
        const tEased = s.labelT < 0.5
          ? 2 * s.labelT * s.labelT
          : 1 - Math.pow(-2 * s.labelT + 2, 2) / 2;

        ctx.font = "500 15px 'Poppins', sans-serif";
        const labelY = HEIGHT * 0.72;

        ctx.save();
        ctx.shadowColor = `rgba(255, 245, 235, ${s.labelOpacity * 0.55})`;
        ctx.shadowBlur = 14;

        if (s.labelT < 1 && s.prevLabel) {
          ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity * (1 - tEased)})`;
          ctx.fillText(s.prevLabel, cx, labelY);
        }
        ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity * tEased})`;
        ctx.fillText(s.shownLabel, cx, labelY);
        ctx.restore();

        // Next-phase cue: fades with labelOpacity so both lines swap together.
        const bt2 = technique as BreathingTechnique;
        const nextPhase = bt2.phases[(s.phaseIndex + 1) % bt2.phases.length];
        ctx.font = "400 12px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity * 0.55})`;
        ctx.fillText("then " + nextPhase.label.toLowerCase(), cx, HEIGHT * 0.755);

        // Persistent technique instruction below the active word.
        // Stays visible the whole session so users never have to
        // remember the rhythm. Drawn without a shadow so it reads as
        // recipe text, not as another "active" element.
        ctx.font = "400 12px 'Poppins', sans-serif";
        ctx.fillStyle = `rgba(255, 255, 255, ${s.labelOpacity * 0.9})`;
        const instructionLines = wrapText(ctx, technique.instructions, WIDTH - 60);
        instructionLines.forEach((line, i) => {
          ctx.fillText(line, cx, HEIGHT * 0.79 + i * 16);
        });
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

      // ── PROGRESS BORDER ──
      if (s.introDone && !s.done) {
        let progress = 0;
        if (!isMindful) {
          const bt = technique as BreathingTechnique;
          const totalPhases = bt.rounds * bt.phases.length;
          const completedPhases = s.round * bt.phases.length + s.phaseIndex;
          const currentPhaseProgress = bt.phases[s.phaseIndex] ? phaseT : 0;
          progress = (completedPhases + currentPhaseProgress) / totalPhases;
        } else {
          const mt = technique as MindfulnessTechnique;
          const currentPromptProgress = mt.prompts[s.promptIndex]
            ? s.promptTime / mt.prompts[s.promptIndex].duration : 0;
          progress = (s.promptIndex + currentPromptProgress) / mt.prompts.length;
        }
        progress = Math.max(0, Math.min(progress, 1));

        // Draw rounded-rect border path, then stroke partial progress
        const r = 16, inset = 1.5, lw = 2;
        const x0 = inset, y0 = inset, w = WIDTH - inset * 2, bh = HEIGHT - inset * 2;
        // Total perimeter of rounded rect
        const straight = 2 * (w - 2 * r) + 2 * (bh - 2 * r);
        const curved = 2 * Math.PI * r;
        const totalLen = straight + curved;
        const progressLen = progress * totalLen;

        ctx.save();
        ctx.lineWidth = lw;
        ctx.strokeStyle = `hsla(${h + 30}, ${Math.min(sat + 40, 80)}%, ${Math.min(l + 35, 70)}%, 0.5)`;
        ctx.lineCap = "round";
        ctx.setLineDash([progressLen, totalLen]);
        ctx.beginPath();
        // Start from top-center, go clockwise
        ctx.moveTo(x0 + r, y0);
        ctx.lineTo(x0 + w - r, y0);
        ctx.arcTo(x0 + w, y0, x0 + w, y0 + r, r);
        ctx.lineTo(x0 + w, y0 + bh - r);
        ctx.arcTo(x0 + w, y0 + bh, x0 + w - r, y0 + bh, r);
        ctx.lineTo(x0 + r, y0 + bh);
        ctx.arcTo(x0, y0 + bh, x0, y0 + bh - r, r);
        ctx.lineTo(x0, y0 + r);
        ctx.arcTo(x0, y0, x0 + r, y0, r);
        ctx.closePath();
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.restore();
      }

      rafRef.current = requestAnimationFrame(tick);
    }
    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [onComplete, WIDTH, HEIGHT, logSession]);

  return (
    <div style={{ width: WIDTH, height: HEIGHT, borderRadius: 16, overflow: "hidden", position: "relative", cursor: "default", userSelect: "none" }}>
      <canvas ref={canvasRef} onClick={togglePause} style={{ width: WIDTH, height: HEIGHT, display: "block", cursor: "pointer" }} />
      <div
        role="status"
        aria-live="polite"
        aria-atomic="true"
        style={{ position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0,0,0,0)", whiteSpace: "nowrap" }}
      >
        {announcedPhase}
      </div>
      {/* Pause/resume icon — centered overlay, only visible during active practice. */}
      {!waiting && (
        <button
          onClick={(e) => { e.stopPropagation(); togglePause(); }}
          title={paused ? "Resume" : "Pause"}
          style={{
            position: "absolute",
            left: "50%", top: "50%",
            transform: "translate(-50%, -50%)",
            width: 48, height: 48,
            display: "flex", alignItems: "center", justifyContent: "center",
            background: paused ? "rgba(255,255,255,0.10)" : "transparent",
            border: paused ? "1px solid rgba(255,255,255,0.18)" : "1px solid transparent",
            color: paused ? "rgba(255,255,255,0.85)" : "rgba(255,255,255,0)",
            cursor: "pointer", borderRadius: "50%", outline: "none",
            transition: "opacity 0.25s ease, background 0.25s ease, border-color 0.25s ease, color 0.25s ease",
            opacity: paused ? 1 : 0,
            pointerEvents: paused ? "auto" : "none",
            zIndex: 9,
            backdropFilter: paused ? "blur(6px)" : "none",
            WebkitBackdropFilter: paused ? "blur(6px)" : "none",
          }}
        >
          {paused ? (
            // Play icon (resume)
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
          ) : (
            // Pause icon (placeholder, hidden when not paused)
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>
          )}
        </button>
      )}
      {/* Pre-session info screen */}
      {waiting && (
        <div style={{
          position: "absolute", inset: 0, zIndex: 5,
          display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center",
          fontFamily: "'Poppins', sans-serif",
          color: "rgba(255, 255, 255, 0.85)",
        }}>
          {/* Frosted blur backdrop */}
          <div
            className={transitioning ? "info-exit-backdrop" : ""}
            style={{
              position: "absolute", inset: 0,
              background: "rgba(10, 8, 20, 0.6)",
              backdropFilter: "blur(20px)", WebkitBackdropFilter: "blur(20px)",
              borderRadius: 16,
            }}
          />
          {/* Content */}
          <div
            className={transitioning ? "info-exit-content" : ""}
            style={{ position: "relative", zIndex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", padding: "0 24px", textAlign: "center" }}
          >
            {/* Compact header */}
            <div className="info-fade-1" style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
              <img
                src="/icons/niyora-logo.png"
                alt="Niyora"
                style={{ width: 18, height: 18, opacity: 0.85 }}
                onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
              />
              <div style={{ fontSize: 13, fontWeight: 500, letterSpacing: "3px", textTransform: "uppercase", color: "rgba(255, 255, 255, 0.7)" }}>
                Niyora
              </div>
            </div>
            {/* Tagline */}
            <div className="info-fade-2" style={{ fontSize: 11, fontWeight: 300, color: "rgba(255, 255, 255, 0.4)", marginBottom: 18 }}>
              Calm in 60 seconds
            </div>
            {/* Big gas-planet ball — color + atmospheric glow reflect today's Niyora Index */}
            {(() => {
              const score = snapshot?.score ?? 100;
              const grad = scoreToBallGradient(score);
              const accent = scoreToOrbAccent(score);
              return (
                <div
                  className="info-fade-3 stress-ball"
                  style={{
                    background: `radial-gradient(circle at 35% 30%, ${grad.highlight} 0%, ${grad.mid} 45%, ${grad.edge} 100%)`,
                    boxShadow: `
                      0 0 28px 4px ${grad.glow},
                      0 0 64px 16px ${grad.glowFaint},
                      0 24px 50px rgba(0, 0, 0, 0.4),
                      inset -22px -18px 36px rgba(0, 0, 0, 0.32),
                      inset 14px 10px 26px rgba(255, 255, 255, 0.10)
                    `,
                    // Drives the cloud whorls in CSS — calm score (>=80) gets
                    // strength 0 so the orb is clean; higher stress brings
                    // visible drift in the matching tier hue.
                    ["--orb-hue" as string]: accent.hue,
                    ["--orb-cloud" as string]: accent.strength,
                  }}
                />
              );
            })()}
            {/* Technique name + subtitle — so users see what they're about
                to do (and decide whether to "Try a different one"). On the
                day's first panel open we swap to a "start your day" framing
                instead, since the technique is forced to Box Breath. */}
            <div className="info-fade-3" style={{ marginTop: 16, textAlign: "center" }}>
              <div style={{
                fontSize: 18,
                fontWeight: 500,
                letterSpacing: "0.5px",
                color: "rgba(255, 255, 255, 0.95)",
                marginBottom: 2,
              }}>
                {showDayStart ? "Start your day with a breath." : stateRef.current.technique.name}
              </div>
              <div style={{
                fontSize: 12,
                fontWeight: 300,
                letterSpacing: "0.3px",
                color: "rgba(255, 255, 255, 0.55)",
              }}>
                {showDayStart ? "60s goes a long way." : stateRef.current.technique.subtitle}
              </div>
            </div>
            <div style={{ height: 18 }} />
            {/* Begin button */}
            <div style={{ position: "relative" }}
              onMouseEnter={btnParticles.startEmitting}
              onMouseLeave={btnParticles.stopEmitting}
            >
              {/* Hover particles */}
              {btnParticles.particles.map(p => (
                <div key={p.id} style={{
                  position: "absolute",
                  left: "50%", top: "50%",
                  width: p.size, height: p.size,
                  borderRadius: "50%",
                  transform: `translate(${p.x - p.size / 2}px, ${p.y - p.size / 2}px)`,
                  background: `radial-gradient(circle, hsla(${p.hue}, 70%, 65%, ${p.opacity}) 0%, hsla(${p.hue}, 60%, 50%, ${p.opacity * 0.4}) 60%, transparent 100%)`,
                  boxShadow: `0 0 ${p.size * 2}px hsla(${p.hue}, 70%, 55%, ${p.opacity * 0.4})`,
                  pointerEvents: "none",
                  zIndex: 0,
                }} />
              ))}
              <button className="info-fade-4" onClick={handleBegin} disabled={transitioning} style={{
                padding: "12px 48px", borderRadius: 28, position: "relative", zIndex: 1,
                background: "linear-gradient(135deg, hsla(270, 50%, 45%, 0.8), hsla(280, 40%, 35%, 0.8))",
                border: "1px solid hsla(270, 40%, 55%, 0.3)",
                color: transitioning ? "transparent" : "rgba(255, 255, 255, 0.9)",
                fontSize: 15, fontWeight: 500, fontFamily: "'Poppins', sans-serif",
                cursor: transitioning ? "default" : "pointer", letterSpacing: "2px", transition: "all 0.3s ease", outline: "none",
                boxShadow: "0 4px 20px hsla(270, 50%, 40%, 0.3), inset 0 1px 0 hsla(270, 60%, 70%, 0.15)",
              }}
                onMouseEnter={(e) => { if (!transitioning) { e.currentTarget.style.background = "linear-gradient(135deg, hsla(270, 55%, 50%, 0.9), hsla(280, 45%, 40%, 0.9))"; e.currentTarget.style.boxShadow = "0 6px 28px hsla(270, 50%, 40%, 0.45), inset 0 1px 0 hsla(270, 60%, 70%, 0.2)"; }}}
                onMouseLeave={(e) => { if (!transitioning) { e.currentTarget.style.background = "linear-gradient(135deg, hsla(270, 50%, 45%, 0.8), hsla(280, 40%, 35%, 0.8))"; e.currentTarget.style.boxShadow = "0 4px 20px hsla(270, 50%, 40%, 0.3), inset 0 1px 0 hsla(270, 60%, 70%, 0.15)"; }}}
              >
                Begin
              </button>
              {/* Expanding orb on click */}
              {transitioning && (
                <div className="begin-orb" style={{
                  left: "50%", top: "50%",
                  marginLeft: -24, marginTop: -24,
                  zIndex: 2,
                }} />
              )}
            </div>
            {/* Secondary action — open the technique picker. */}
            <button
              className="info-fade-4 info-secondary-btn"
              onClick={() => setShowPicker(true)}
              disabled={transitioning}
            >
              Try a different one
            </button>
          </div>
        </div>
      )}
      {/* Pick-from-list overlay (only over the info screen). */}
      {waiting && showPicker && (
        <div className="picker-overlay">
          <div className="picker-backdrop" />
          <div className="picker-content">
            <button
              className="picker-back"
              onClick={() => setShowPicker(false)}
              title="Back"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M19 12H5M12 19l-7-7 7-7" />
              </svg>
            </button>
            <div className="picker-title">Choose a practice</div>

            <div className="picker-section">
              <div className="picker-section-label">Breathing</div>
              {breathingList.map((t) => {
                const locked = isLocked(t);
                return (
                  <button
                    key={t.name}
                    className={`picker-row ${locked ? "is-locked niyora-tip" : ""}`}
                    onClick={() => !locked && selectTechnique(t)}
                    aria-disabled={locked}
                    data-tooltip={locked ? lockTooltip(t) : undefined}
                  >
                    <div className="picker-row-text">
                      <div className="picker-row-name">{t.name}</div>
                      <div className="picker-row-sub">{t.subtitle}</div>
                    </div>
                    {locked && (
                      <span className="picker-lock">
                        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                          <rect x="3" y="11" width="18" height="11" rx="2" />
                          <path d="M7 11V7a5 5 0 0110 0v4" />
                        </svg>
                      </span>
                    )}
                  </button>
                );
              })}
            </div>

            <div className="picker-section">
              <div className="picker-section-label">Mindfulness</div>
              {mindfulnessList.map((t) => {
                const locked = isLocked(t);
                return (
                  <button
                    key={t.name}
                    className={`picker-row ${locked ? "is-locked niyora-tip" : ""}`}
                    onClick={() => !locked && selectTechnique(t)}
                    aria-disabled={locked}
                    data-tooltip={locked ? lockTooltip(t) : undefined}
                  >
                    <div className="picker-row-text">
                      <div className="picker-row-name">{t.name}</div>
                      <div className="picker-row-sub">{t.subtitle}</div>
                    </div>
                    {locked && (
                      <span className="picker-lock">
                        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                          <rect x="3" y="11" width="18" height="11" rx="2" />
                          <path d="M7 11V7a5 5 0 0110 0v4" />
                        </svg>
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      )}
      {/* Top-right: Track selector (left of mute, only on pre-session screen) */}
      {waiting && (
        <div ref={trackMenuRef} style={{ position: "absolute", top: 8, right: 68, zIndex: 11 }}>
          <button
            onClick={() => setShowTrackMenu((v) => !v)}
            className="niyora-tip"
            data-tooltip="Soundscape"
            title="Soundscape"
            style={{
              width: 26, height: 26,
              display: "flex", alignItems: "center", justifyContent: "center",
              background: showTrackMenu ? "rgba(255,255,255,0.12)" : "transparent",
              border: "none", color: showTrackMenu ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.55)",
              cursor: "pointer", borderRadius: 6, transition: "all 0.2s ease", outline: "none",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.95)"; e.currentTarget.style.background = "rgba(255,255,255,0.12)"; }}
            onMouseLeave={(e) => { if (!showTrackMenu) { e.currentTarget.style.color = "rgba(255,255,255,0.55)"; e.currentTarget.style.background = "transparent"; } }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M9 18V5l12-2v13" />
              <circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
            </svg>
          </button>
          {showTrackMenu && (
            <div style={{
              position: "absolute", top: 30, right: 0,
              background: "rgba(20, 18, 30, 0.92)",
              backdropFilter: "blur(16px)", WebkitBackdropFilter: "blur(16px)",
              border: "1px solid rgba(255,255,255,0.12)",
              borderRadius: 8, padding: "4px 0",
              minWidth: 120, boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
            }}>
              {([["random", "Random"], ["serene", "Serene"], ["ocean", "Ocean"], ["forest", "Forest"]] as const).map(([key, label]) => (
                <button
                  key={key}
                  onClick={() => {
                    setPreferredTrack(key);
                    setShowTrackMenu(false);
                    invoke("set_preferred_track", { track: key }).catch(() => {});
                  }}
                  style={{
                    display: "flex", alignItems: "center", gap: 8,
                    width: "100%", padding: "6px 12px",
                    background: "transparent", border: "none",
                    color: preferredTrack === key ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.6)",
                    fontSize: 12, fontFamily: "'Poppins', sans-serif", fontWeight: preferredTrack === key ? 500 : 300,
                    cursor: "pointer", textAlign: "left", outline: "none",
                    transition: "background 0.15s ease",
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.08)"; }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                >
                  <span style={{ width: 12, textAlign: "center", fontSize: 10, opacity: preferredTrack === key ? 1 : 0 }}>&#10003;</span>
                  {label}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      {/* Top-right: Mute/unmute (left of close) */}
      <button onClick={toggleMute} className="niyora-tip" data-tooltip={muted ? "Unmute" : "Mute"} title={muted ? "Unmute" : "Mute"} style={{
        position: "absolute", top: 8, right: 38, width: 26, height: 26,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: "transparent", border: "none", color: "rgba(255,255,255,0.55)",
        cursor: "pointer", borderRadius: 6, zIndex: 10, transition: "all 0.2s ease", outline: "none",
      }}
        onMouseEnter={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.95)"; e.currentTarget.style.background = "rgba(255,255,255,0.12)"; }}
        onMouseLeave={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.55)"; e.currentTarget.style.background = "transparent"; }}
      >
        {muted ? (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5L6 9H2v6h4l5 4V5z" /><line x1="23" y1="9" x2="17" y2="15" /><line x1="17" y1="9" x2="23" y2="15" /></svg>
        ) : (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5L6 9H2v6h4l5 4V5z" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
        )}
      </button>
      {/* Top-right: Close */}
      <button onClick={handleDone} className="niyora-tip" data-tooltip="Close" title="Close" style={{
        position: "absolute", top: 8, right: 8, width: 26, height: 26,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: "transparent", border: "none", color: "rgba(255,255,255,0.55)",
        cursor: "pointer", borderRadius: 6, zIndex: 10, transition: "all 0.2s ease", outline: "none",
      }}
        onMouseEnter={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.95)"; e.currentTarget.style.background = "rgba(255,255,255,0.12)"; }}
        onMouseLeave={(e) => { e.currentTarget.style.color = "rgba(255,255,255,0.55)"; e.currentTarget.style.background = "transparent"; }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"><path d="M2 2l8 8M10 2l-8 8" /></svg>
      </button>
    </div>
  );
}
