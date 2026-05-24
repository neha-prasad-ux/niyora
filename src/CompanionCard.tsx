import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CompanionHrvChart } from "./CompanionHrvChart";

interface PairedDevice {
  client_id: string;
  client_name: string;
  paired_at: string;
  last_seen: string | null;
  last_window_sent_at: string | null;
}

interface CompanionStatus {
  running: boolean;
  server_name: string;
  host: string | null;
  port: number | null;
  paired_devices: PairedDevice[];
  pairing_active: boolean;
  pairing_seconds_remaining: number | null;
}

interface PairingQr {
  payload_b64: string;
  qr_svg: string;
  seconds_remaining: number;
}

/** Human-friendly "2 min ago" / "yesterday" / "just now" for an RFC3339
 * string, with second-by-second freshness · used for "Last window sent" so
 * the user can tell the loop is alive without staring at a raw timestamp. */
function relativeTime(iso: string | null): string {
  if (!iso) return "never yet";
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "never yet";
  const seconds = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24);
  return `${days} d ago`;
}

/** "Connect your iPhone" Soul-panel card. Renders only on platforms that
 * expose the companion commands (today: macOS) · on Windows the initial
 * invoke rejects and the component renders nothing, keeping the rest of
 * My Soul intact. */
export function CompanionCard() {
  const [status, setStatus] = useState<CompanionStatus | null>(null);
  const [available, setAvailable] = useState<boolean | null>(null);
  const [pairing, setPairing] = useState<PairingQr | null>(null);
  const [error, setError] = useState<string | null>(null);
  const tickRef = useRef<number | null>(null);

  // Initial fetch + state-event subscription. If the invoke rejects (no
  // companion commands registered, e.g. Windows build), mark the feature
  // unavailable so we render nothing.
  useEffect(() => {
    let cancelled = false;
    invoke<CompanionStatus>("companion_status")
      .then((s) => {
        if (cancelled) return;
        setStatus(s);
        setAvailable(true);
      })
      .catch(() => {
        if (cancelled) return;
        setAvailable(false);
      });

    const unlistenPromise = listen<CompanionStatus>(
      "companion://state",
      (event) => {
        if (cancelled) return;
        setStatus(event.payload);
      },
    );

    return () => {
      cancelled = true;
      unlistenPromise.then((u) => u()).catch(() => {});
    };
  }, []);

  // Local countdown for the pairing modal. Backend is the source of truth,
  // but ticking locally every second gives a smooth seconds_remaining
  // without an IPC round trip per second.
  useEffect(() => {
    if (pairing === null) {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
      return;
    }
    tickRef.current = window.setInterval(() => {
      setPairing((p) => {
        if (p === null) return null;
        if (p.seconds_remaining <= 1) {
          // Expired locally · cancel server side too so the next start is
          // clean. The user can simply tap Pair again.
          invoke("companion_cancel_pairing").catch(() => {});
          return null;
        }
        return { ...p, seconds_remaining: p.seconds_remaining - 1 };
      });
    }, 1000);
    return () => {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
    };
  }, [pairing !== null]);

  if (available === false) return null;
  if (available === null) return null;

  const devices = status?.paired_devices ?? [];

  return (
    <>
      <div className="soul-card soul-card-cta">
        <div className="soul-cta-title">
          Connect your iPhone
          <span className="soul-beta-pill">Beta</span>
        </div>

        {devices.length === 0 ? (
          <div className="soul-cta-blurb">
            Early access. Pair your iPhone to see whether breathing actually
            moves your heart rate variability. Stays on your devices, never
            the cloud.
          </div>
        ) : (
          <div className="companion-devices">
            {devices.map((d) => (
              <div key={d.client_id} className="companion-device">
                <div className="companion-device-row">
                  <span className="companion-device-dot" />
                  <span className="companion-device-name">{d.client_name}</span>
                  <button
                    className="companion-unpair"
                    onClick={() => {
                      invoke("companion_unpair", { clientId: d.client_id }).catch((e) =>
                        setError(String(e)),
                      );
                    }}
                  >
                    Unpair
                  </button>
                </div>
                <div className="companion-device-meta">
                  Last sent {relativeTime(d.last_window_sent_at)}
                </div>
              </div>
            ))}
          </div>
        )}

        <button
          className="soul-cta-btn"
          onClick={() => {
            setError(null);
            invoke<PairingQr>("companion_start_pairing")
              .then((qr) => setPairing(qr))
              .catch((e) => setError(String(e)));
          }}
        >
          {devices.length === 0 ? "Pair your iPhone" : "Pair another iPhone"}
        </button>

        {devices.length > 0 && <CompanionHrvChart />}

        {error !== null && <div className="companion-error">{error}</div>}
      </div>

      {pairing !== null && (
        <PairModal
          qr={pairing}
          onCancel={() => {
            invoke("companion_cancel_pairing").catch(() => {});
            setPairing(null);
          }}
        />
      )}
    </>
  );
}

function PairModal({ qr, onCancel }: { qr: PairingQr; onCancel: () => void }) {
  return (
    <div className="companion-modal-backdrop" onClick={onCancel}>
      <div className="companion-modal" onClick={(e) => e.stopPropagation()}>
        <div className="companion-modal-title">Pair your iPhone</div>
        <ol className="companion-modal-steps">
          <li>Open Niyora Companion on your iPhone.</li>
          <li>Tap Connect to Mac.</li>
          <li>Point the camera at this QR.</li>
        </ol>
        <div
          className="companion-qr"
          dangerouslySetInnerHTML={{ __html: qr.qr_svg }}
        />
        <div className="companion-modal-timer">
          Expires in {formatRemaining(qr.seconds_remaining)}
        </div>
        <button className="companion-modal-cancel" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

function formatRemaining(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
