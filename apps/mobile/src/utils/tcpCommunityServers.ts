export interface TcpCommunityServer {
  name: string;
  host: string;
  port: number;
  isBootstrap?: boolean;
}

// Mirrors the known community TCP server list used by Columba.
export const TCP_COMMUNITY_SERVERS: TcpCommunityServer[] = [
  { name: "Beleth RNS Hub", host: "rns.beleth.net", port: 4242, isBootstrap: true },
  { name: "Quad4 TCP Node 1", host: "rns.quad4.io", port: 4242, isBootstrap: true },
  { name: "FireZen", host: "firezen.com", port: 4242, isBootstrap: true },
  { name: "g00n.cloud Hub", host: "dfw.us.g00n.cloud", port: 6969 },
  { name: "interloper node", host: "intr.cx", port: 4242 },
  {
    name: "interloper node (Tor)",
    host: "intrcxv4fa72e5ovler5dpfwsiyuo34tkcwfy5snzstxkhec75okowqd.onion",
    port: 4242,
  },
  { name: "Jon's Node", host: "rns.jlamothe.net", port: 4242 },
  { name: "noDNS1", host: "202.61.243.41", port: 4965 },
  { name: "noDNS2", host: "193.26.158.230", port: 4965 },
  { name: "NomadNode SEAsia TCP", host: "rns.jaykayenn.net", port: 4242 },
  { name: "0rbit-Net", host: "93.95.227.8", port: 49952 },
  { name: "Quad4 TCP Node 2", host: "rns2.quad4.io", port: 4242 },
  { name: "Quortal TCP Node", host: "reticulum.qortal.link", port: 4242 },
  { name: "R-Net TCP", host: "istanbul.reserve.network", port: 9034 },
  { name: "RNS bnZ-NODE01", host: "node01.rns.bnz.se", port: 4242 },
  { name: "RNS COMSEC-RD", host: "80.78.23.249", port: 4242 },
  { name: "RNS HAM RADIO", host: "135.125.238.229", port: 4242 },
  { name: "RNS Testnet StoppedCold", host: "rns.stoppedcold.com", port: 4242 },
  { name: "RNS_Transport_US-East", host: "45.77.109.86", port: 4965 },
  { name: "SparkN0de", host: "aspark.uber.space", port: 44860 },
  { name: "Tidudanka.com", host: "reticulum.tidudanka.com", port: 37500 },
];

export function toTcpEndpoint(server: TcpCommunityServer): string {
  return `${server.host}:${server.port}`;
}
