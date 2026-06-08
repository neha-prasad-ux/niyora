//! Noise XX handshake + Short Authentication String (SAS) for the iPhone
//! pairing rework. This is the trust layer that replaces the old
//! QR-delivered secret + HMAC-SHA256 challenge.
//!
//! Why XX: neither side knows the other's key up front (no QR to carry it),
//! so both static public keys are exchanged live during the handshake and
//! mutually authenticated. The handshake transcript hash binds every byte
//! both peers sent, so deriving a short number from it gives a Short
//! Authentication String: if a same-wifi man-in-the-middle ran two separate
//! handshakes, the two transcripts differ and the two numbers will not
//! match. The human glancing at both screens is what defeats the MITM; the
//! human clicking Allow on the Mac is what authorises an unknown phone.
//!
//! Suite: `Noise_XX_25519_ChaChaPoly_SHA256`. SHA256 (not BLAKE2s) so the
//! iOS side can implement the same suite with CryptoKit primitives
//! (Curve25519, ChaChaPoly, SHA256) without a third-party Noise crate.
//!
//! Wire shape:
//!
//! ```text
//!   ->  e                 (handshake msg 1)
//!   <-  e, ee, s, es      (handshake msg 2)
//!   ->  s, se             (handshake msg 3)
//!   [both derive SAS from the handshake hash]
//!   <-> encrypted NDJSON frames (the existing app protocol, now sealed)
//! ```
//!
//! Every frame on the wire (handshake and transport alike) is a 2-byte
//! big-endian length prefix followed by that many bytes. Noise messages are
//! at most 65535 bytes, so the length always fits in a `u16`.

use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The one cipher suite both ends speak. Kept as a `const` so the iOS team
/// has a single source of truth for the string they must parse.
pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

/// Max Noise message size. The transport tag adds 16 bytes over the
/// plaintext; handshake messages are well under this. Used to size scratch
/// buffers.
const MAX_NOISE_MSG: usize = 65535;

/// A Curve25519 static keypair. `private` is the Mac's long-term identity
/// secret (stored in the Keychain); `public` is what the phone records to
/// recognise this Mac on later reconnects, and vice versa.
#[derive(Clone)]
pub struct StaticKeypair {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

/// Result of a completed handshake.
pub struct Handshake {
    /// Sealed channel for the rest of the connection.
    pub transport: TransportState,
    /// The peer's static public key. On a fresh pair the Mac stores this as
    /// the phone's identity; on reconnect it is compared against the stored
    /// value so a phone cannot impersonate another's `client_id`.
    pub remote_static: Vec<u8>,
    /// Full handshake transcript hash. The SAS is derived from this; also
    /// exposed for channel binding and tests.
    pub handshake_hash: Vec<u8>,
    /// The number both screens show. Six grouped digits, e.g. `"123 456"`.
    pub sas: String,
}

/// Generate a fresh static keypair for this suite.
pub fn generate_static_keypair() -> Result<StaticKeypair, String> {
    let params: NoiseParams = NOISE_PARAMS
        .parse()
        .map_err(|e| format!("noise params: {e}"))?;
    let kp = Builder::new(params)
        .generate_keypair()
        .map_err(|e| format!("generate keypair: {e}"))?;
    Ok(StaticKeypair {
        private: kp.private,
        public: kp.public,
    })
}

/// Derive the Short Authentication String from the handshake transcript
/// hash. Both peers compute this from the identical hash, so a man-in-the
/// middle running two separate handshakes yields two different numbers and
/// the human comparing the screens catches it.
///
/// Six decimal digits, rendered grouped (`"123 456"`) so they are easy to
/// read aloud and compare at a glance.
pub fn sas_from_hash(handshake_hash: &[u8]) -> String {
    let mut b = [0u8; 4];
    b.copy_from_slice(&handshake_hash[..4]);
    let n = u32::from_be_bytes(b) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// Run the handshake as the responder (the Mac). Reads msg 1, writes msg 2,
/// reads msg 3, then returns the sealed transport plus the peer identity and
/// SAS. `local_private` is the Mac's static secret key.
pub async fn handshake_responder<S>(
    stream: &mut S,
    local_private: &[u8],
) -> Result<Handshake, String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut hs = build(local_private, false)?;
    let mut buf = vec![0u8; MAX_NOISE_MSG];

    // <- e
    let msg1 = read_frame(stream).await.map_err(|e| format!("read e: {e}"))?;
    hs.read_message(&msg1, &mut buf)
        .map_err(|e| format!("noise msg1: {e}"))?;

    // -> e, ee, s, es
    let n = hs
        .write_message(&[], &mut buf)
        .map_err(|e| format!("noise msg2: {e}"))?;
    write_frame(stream, &buf[..n])
        .await
        .map_err(|e| format!("write msg2: {e}"))?;

    // <- s, se
    let msg3 = read_frame(stream)
        .await
        .map_err(|e| format!("read msg3: {e}"))?;
    hs.read_message(&msg3, &mut buf)
        .map_err(|e| format!("noise msg3: {e}"))?;

    finish(hs)
}

/// Run the handshake as the initiator (the phone). Mirror of
/// `handshake_responder`. Kept here as the byte-exact reference for the iOS
/// implementation and exercised by the round-trip tests.
pub async fn handshake_initiator<S>(
    stream: &mut S,
    local_private: &[u8],
) -> Result<Handshake, String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut hs = build(local_private, true)?;
    let mut buf = vec![0u8; MAX_NOISE_MSG];

    // -> e
    let n = hs
        .write_message(&[], &mut buf)
        .map_err(|e| format!("noise msg1: {e}"))?;
    write_frame(stream, &buf[..n])
        .await
        .map_err(|e| format!("write e: {e}"))?;

    // <- e, ee, s, es
    let msg2 = read_frame(stream)
        .await
        .map_err(|e| format!("read msg2: {e}"))?;
    hs.read_message(&msg2, &mut buf)
        .map_err(|e| format!("noise msg2: {e}"))?;

    // -> s, se
    let n = hs
        .write_message(&[], &mut buf)
        .map_err(|e| format!("noise msg3: {e}"))?;
    write_frame(stream, &buf[..n])
        .await
        .map_err(|e| format!("write msg3: {e}"))?;

    finish(hs)
}

fn build(local_private: &[u8], initiator: bool) -> Result<HandshakeState, String> {
    let params: NoiseParams = NOISE_PARAMS
        .parse()
        .map_err(|e| format!("noise params: {e}"))?;
    let builder = Builder::new(params).local_private_key(local_private);
    if initiator {
        builder
            .build_initiator()
            .map_err(|e| format!("build initiator: {e}"))
    } else {
        builder
            .build_responder()
            .map_err(|e| format!("build responder: {e}"))
    }
}

/// Snapshot the peer identity + SAS off the finished handshake, then seal it
/// into transport mode. The hash and remote-static reads must happen before
/// `into_transport_mode` consumes the handshake state.
fn finish(hs: HandshakeState) -> Result<Handshake, String> {
    let remote_static = hs
        .get_remote_static()
        .ok_or_else(|| "handshake completed without a remote static key".to_string())?
        .to_vec();
    let handshake_hash = hs.get_handshake_hash().to_vec();
    let sas = sas_from_hash(&handshake_hash);
    let transport = hs
        .into_transport_mode()
        .map_err(|e| format!("into transport: {e}"))?;
    Ok(Handshake {
        transport,
        remote_static,
        handshake_hash,
        sas,
    })
}

/// Seal one plaintext frame (an NDJSON app message) and write it framed.
pub async fn send_encrypted<S>(
    stream: &mut S,
    transport: &mut TransportState,
    plaintext: &[u8],
) -> Result<(), String>
where
    S: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; plaintext.len() + 16];
    let n = transport
        .write_message(plaintext, &mut buf)
        .map_err(|e| format!("seal: {e}"))?;
    write_frame(stream, &buf[..n])
        .await
        .map_err(|e| format!("write sealed: {e}"))
}

/// Read one framed sealed message and return its plaintext.
pub async fn recv_encrypted<S>(
    stream: &mut S,
    transport: &mut TransportState,
) -> Result<Vec<u8>, String>
where
    S: AsyncRead + Unpin,
{
    let frame = read_frame(stream)
        .await
        .map_err(|e| format!("read sealed: {e}"))?;
    let mut buf = vec![0u8; frame.len()];
    let n = transport
        .read_message(&frame, &mut buf)
        .map_err(|e| format!("open: {e}"))?;
    buf.truncate(n);
    Ok(buf)
}

async fn write_frame<W>(w: &mut W, payload: &[u8]) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let len = u16::try_from(payload.len()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "frame exceeds 65535 bytes")
    })?;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await?;
    Ok(())
}

async fn read_frame<R>(r: &mut R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 2];
    r.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[test]
    fn sas_is_six_grouped_digits_from_first_four_bytes() {
        // 0x0001E240 == 123456 -> "123 456".
        assert_eq!(sas_from_hash(&[0x00, 0x01, 0xE2, 0x40, 0xFF, 0xFF]), "123 456");
        // 0x000F4240 == 1_000_000 -> wraps to 0 -> "000 000".
        assert_eq!(sas_from_hash(&[0x00, 0x0F, 0x42, 0x40]), "000 000");
    }

    #[tokio::test]
    async fn xx_handshake_matches_sas_identities_and_transports_both_ways() {
        let init_kp = generate_static_keypair().unwrap();
        let resp_kp = generate_static_keypair().unwrap();
        let (mut a, mut b) = duplex(8192);

        let initiator = async {
            let mut hs = handshake_initiator(&mut a, &init_kp.private).await.unwrap();
            send_encrypted(&mut a, &mut hs.transport, br#"{"type":"identify"}"#)
                .await
                .unwrap();
            let got = recv_encrypted(&mut a, &mut hs.transport).await.unwrap();
            (hs.sas, hs.remote_static, got)
        };
        let responder = async {
            let mut hs = handshake_responder(&mut b, &resp_kp.private).await.unwrap();
            let got = recv_encrypted(&mut b, &mut hs.transport).await.unwrap();
            send_encrypted(&mut b, &mut hs.transport, br#"{"type":"authed"}"#)
                .await
                .unwrap();
            (hs.sas, hs.remote_static, got)
        };

        let ((i_sas, i_remote, i_got), (r_sas, r_remote, r_got)) =
            tokio::join!(initiator, responder);

        // Same number on both screens.
        assert_eq!(i_sas, r_sas);
        assert_eq!(i_sas.len(), 7, "NNN NNN");
        // Each side learned the other's real static key.
        assert_eq!(i_remote, resp_kp.public);
        assert_eq!(r_remote, init_kp.public);
        // Sealed app frames round-trip in both directions.
        assert_eq!(i_got, br#"{"type":"authed"}"#);
        assert_eq!(r_got, br#"{"type":"identify"}"#);
    }

    #[tokio::test]
    async fn man_in_the_middle_yields_mismatched_transcripts() {
        // A MITM cannot share one handshake with both ends: it must run a
        // separate handshake on each leg. The two transcripts differ, so the
        // two SAS numbers differ, so the human catches it.
        let real_i = generate_static_keypair().unwrap();
        let real_r = generate_static_keypair().unwrap();
        let mitm = generate_static_keypair().unwrap();

        let (mut a1, mut b1) = duplex(8192);
        let leg1 = async {
            let i = handshake_initiator(&mut a1, &real_i.private).await.unwrap();
            i.handshake_hash
        };
        let leg1_m = async {
            handshake_responder(&mut b1, &mitm.private)
                .await
                .unwrap()
                .handshake_hash
        };
        let (hash_real_i, _hash_mitm_1) = tokio::join!(leg1, leg1_m);

        let (mut a2, mut b2) = duplex(8192);
        let leg2_m = async {
            handshake_initiator(&mut a2, &mitm.private)
                .await
                .unwrap()
                .handshake_hash
        };
        let leg2 = async {
            let r = handshake_responder(&mut b2, &real_r.private).await.unwrap();
            r.handshake_hash
        };
        let (_hash_mitm_2, hash_real_r) = tokio::join!(leg2_m, leg2);

        assert_ne!(
            hash_real_i, hash_real_r,
            "the two legs of a MITM must not share a transcript"
        );
        assert_ne!(sas_from_hash(&hash_real_i), sas_from_hash(&hash_real_r));
    }

    #[tokio::test]
    async fn tampered_static_key_is_rejected_on_reconnect_compare() {
        // Simulates the reconnect check: after a clean handshake the Mac has
        // the phone's real static key. A different phone (different key)
        // completing the handshake presents a different remote_static, so the
        // stored-key comparison fails.
        let real_phone = generate_static_keypair().unwrap();
        let other_phone = generate_static_keypair().unwrap();
        let mac = generate_static_keypair().unwrap();

        let (mut a, mut b) = duplex(8192);
        let phone = async {
            handshake_initiator(&mut a, &other_phone.private)
                .await
                .unwrap()
                .remote_static
        };
        let mac_side = async {
            handshake_responder(&mut b, &mac.private)
                .await
                .unwrap()
                .remote_static
        };
        let (_phone_saw, mac_saw_remote) = tokio::join!(phone, mac_side);

        assert_eq!(mac_saw_remote, other_phone.public);
        assert_ne!(
            mac_saw_remote, real_phone.public,
            "an unknown phone must not match the stored identity"
        );
    }
}
