import { useEffect, useRef } from "react";

interface Props {
  onContinue: () => void;
}

interface Flake {
  x: number;
  y: number;
  r: number;
  vy: number;
  drift: number;
  driftSpeed: number;
  alpha: number;
}

/** Soft snowfall canvas. Particles drift down with a gentle horizontal sway,
 * matching the breathing-canvas vocabulary without sharing its state. */
function useSnowfall(canvasRef: React.RefObject<HTMLCanvasElement>) {
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const W = rect.width;
    const H = rect.height;
    const flakes: Flake[] = Array.from({ length: 60 }, () => ({
      x: Math.random() * W,
      y: Math.random() * H - H,
      r: 1 + Math.random() * 2.4,
      vy: 18 + Math.random() * 28,
      drift: Math.random() * Math.PI * 2,
      driftSpeed: 0.4 + Math.random() * 0.6,
      alpha: 0.45 + Math.random() * 0.45,
    }));

    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      ctx.clearRect(0, 0, W, H);
      for (const f of flakes) {
        f.y += f.vy * dt;
        f.drift += f.driftSpeed * dt;
        const x = f.x + Math.sin(f.drift) * 8;
        if (f.y > H + 4) {
          f.y = -4;
          f.x = Math.random() * W;
        }
        ctx.beginPath();
        ctx.arc(x, f.y, f.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(255, 255, 255, ${f.alpha})`;
        ctx.shadowColor = "rgba(255, 255, 255, 0.6)";
        ctx.shadowBlur = 6;
        ctx.fill();
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [canvasRef]);
}

export default function FirstSessionAha({ onContinue }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useSnowfall(canvasRef);

  return (
    <div className="niyora-aha">
      <div className="aha-backdrop" />
      <canvas ref={canvasRef} className="aha-snow" />
      <div className="aha-content">
        <div className="aha-eyebrow">Your first session</div>
        <div className="aha-headline">Happy first session.</div>
        <div className="aha-sub">Your body is already thanking you.</div>
        <button className="aha-continue" onClick={onContinue}>Continue</button>
      </div>
    </div>
  );
}
