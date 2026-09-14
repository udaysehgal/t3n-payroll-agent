import "dotenv/config";
import {
  setEnvironment,
  fetchTrustedManifest,
  loadWasmComponent,
  createDefaultHandlers,
  createEthAuthInput,
  eth_get_address,
  getNodeUrl,
  T3nClient,
  type Environment,
} from "@terminal3/t3n-sdk";

const PRIVATE_KEY = process.env.T3N_API_KEY;
const ENV: Environment = (process.env.T3N_ENVIRONMENT as Environment) || "testnet";

if (!PRIVATE_KEY) {
  throw new Error("T3N_API_KEY is not set (expected in .env)");
}

export async function connect() {
  setEnvironment(ENV);
  const baseUrl = getNodeUrl();
  const trustAnchor = await fetchTrustedManifest(ENV);
  const wasmComponent = await loadWasmComponent();

  const address = eth_get_address(PRIVATE_KEY!);
  const handlers = createDefaultHandlers(baseUrl, trustAnchor);
  handlers.EthSign = (await import("@terminal3/t3n-sdk")).metamask_sign(
    address,
    undefined,
    PRIVATE_KEY,
  );

  const t3n = new T3nClient({
    baseUrl,
    wasmComponent,
    trustAnchor,
    handlers,
  });

  await t3n.handshake();
  const did = await t3n.authenticate(createEthAuthInput(address, { ethDerived: true }));

  return { t3n, did: did.toString(), baseUrl, trustAnchor, address };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  connect()
    .then(({ did, baseUrl }) => {
      console.log(`Environment: ${ENV}`);
      console.log(`Node: ${baseUrl}`);
      console.log(`Connected as: ${did}`);
    })
    .catch((err) => {
      console.error("Connect failed:", err.message ?? err);
      process.exit(1);
    });
}
