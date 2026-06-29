const NOISY_NATIVE_LOG_PATTERNS = [
  "[tp-diag] inbound_packet",
  "[iface][rx]",
  "[announceReceived]",
  "[packetReceived]",
  "[link][maintain]",
  "[lxmf][events] link activation retry",
  "[lxmf][queue]",
  "[lxmf][events][sdk] attempting send",
  "[lxmf][mission] resolved send",
  " is now reachable over ",
  "repeat link request",
  "RNode BLE packet serialize failed",
];

export function nativeLogShouldAppendToUi(level: string, message: string): boolean {
  const normalizedLevel = level.trim().toLowerCase();
  const normalizedMessage = message.trim();

  if (normalizedLevel === "error") {
    return true;
  }

  return !NOISY_NATIVE_LOG_PATTERNS.some((pattern) => normalizedMessage.includes(pattern));
}
