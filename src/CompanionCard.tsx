import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface PairedDevice {
  client_id: string;
  client_name: string;
  paired_at: string;
  last_seen: string | null;
  last_window_sent_at: string | null;
}

interface PairingRequest {
  client_id: string;
  client_name: string;
  sas: string;
}

interface CompanionStatus {
  running: boolean;
  server_name: string;
  host: string | null;
  port: number | null;
  paired_devices: PairedDevice[];
  pairing_active: boolean;
  pairing_seconds_remaining: number | null;
  pending_requests: PairingRequest[];
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
  const [error, setError] = useState<string | null>(null);
  const [secondsLeft, setSecondsLeft] = useState<number | null>(null);
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

  // Reseed the local countdown whenever the backend reports a fresh window
  // remaining. Ticking locally avoids an IPC round trip per second.
  const pairingActive = status?.pairing_active ?? false;
  const backendRemaining = status?.pairing_seconds_remaining ?? null;
  useEffect(() => {
    if (pairingActive && backendRemaining !== null) {
      setSecondsLeft(backendRemaining);
    } else {
      setSecondsLeft(null);
    }
  }, [pairingActive, backendRemaining]);

  useEffect(() => {
    if (secondsLeft === null) {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
      return;
    }
    tickRef.current = window.setInterval(() => {
      setSecondsLeft((s) => {
        if (s === null) return null;
        if (s <= 1) {
          invoke("companion_cancel_pairing").catch(() => {});
          return null;
        }
        return s - 1;
      });
    }, 1000);
    return () => {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
    };
  }, [secondsLeft === null]);

  if (available === false) return null;
  if (available === null) return null;

  const devices = status?.paired_devices ?? [];
  const request = status?.pending_requests?.[0] ?? null;
  const showModal = pairingActive || request !== null;

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
            invoke("companion_start_pairing").catch((e) => setError(String(e)));
          }}
        >
          {devices.length === 0 ? "Pair your iPhone" : "Pair another iPhone"}
        </button>

        {error !== null && <div className="companion-error">{error}</div>}
      </div>

      {showModal && (
        <PairModal
          request={request}
          secondsLeft={secondsLeft}
          onCancel={() => {
            invoke("companion_cancel_pairing").catch(() => {});
          }}
          onApprove={(clientId) => {
            invoke("companion_approve_pairing", { clientId }).catch((e) =>
              setError(String(e)),
            );
          }}
          onReject={(clientId) => {
            invoke("companion_reject_pairing", { clientId }).catch(() => {});
          }}
        />
      )}
    </>
  );
}

function PairModal({
  request,
  secondsLeft,
  onCancel,
  onApprove,
  onReject,
}: {
  request: PairingRequest | null;
  secondsLeft: number | null;
  onCancel: () => void;
  onApprove: (clientId: string) => void;
  onReject: (clientId: string) => void;
}) {
  // Two states drive one modal: searching for a phone, then approving the
  // one that connected. The backend window stays open until the user acts.
  if (request !== null) {
    return (
      <div className="companion-modal-backdrop">
        <div className="companion-modal" onClick={(e) => e.stopPropagation()}>
          <div className="companion-modal-title">
            {request.client_name} wants to connect
          </div>
          <div className="companion-sas">{request.sas}</div>
          <div className="companion-modal-sub">
            This number should match the one on your iPhone.
          </div>
          <div className="companion-modal-actions">
            <button
              className="companion-modal-cancel"
              onClick={() => onReject(request.client_id)}
            >
              Not now
            </button>
            <button
              className="soul-cta-btn"
              onClick={() => onApprove(request.client_id)}
            >
              Allow
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="companion-modal-backdrop" onClick={onCancel}>
      <div className="companion-modal" onClick={(e) => e.stopPropagation()}>
        <div className="companion-modal-title">Pair your iPhone</div>
        <div className="companion-modal-sub">
          Open Niyora on your iPhone and tap this Mac.
        </div>
        <div className="companion-searching">
          <span className="companion-searching-dot" />
          Looking for your iPhone
        </div>
        {secondsLeft !== null && (
          <div className="companion-modal-timer">
            Open for {formatRemaining(secondsLeft)}
          </div>
        )}
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
