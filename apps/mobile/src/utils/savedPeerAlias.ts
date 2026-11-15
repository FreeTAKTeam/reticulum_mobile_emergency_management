/**
 * True when `destination` is the app-hash alias of a peer already saved under its
 * LXMF hash. The runtime can report that alias as a saved peer of its own; saving
 * it again fights the alias migration and the saved list never settles.
 */
export function isSavedPeerAppAlias(
  destination: string,
  appDestinationByIdentity: Record<string, string>,
  lxmfDestinationByIdentity: Record<string, string>,
  savedByDestination: Record<string, unknown>,
): boolean {
  return Object.keys(appDestinationByIdentity).some((identity) =>
    appDestinationByIdentity[identity] === destination
    && Boolean(savedByDestination[lxmfDestinationByIdentity[identity] ?? ""]));
}
