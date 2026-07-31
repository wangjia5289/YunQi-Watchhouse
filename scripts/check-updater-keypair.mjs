import { spawnSync } from "node:child_process";
import { createHash, createPublicKey, verify } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const privateKey = process.env.TAURI_SIGNING_PRIVATE_KEY?.trim();
const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY?.trim();
if (!privateKey || !publicKey) {
  throw new Error("TAURI_SIGNING_PRIVATE_KEY and TAURI_UPDATER_PUBLIC_KEY are required");
}

function decodeEnvelope(value, label) {
  const decoded = Buffer.from(value, "base64").toString("utf8").trim();
  const lines = decoded.split(/\r?\n/);
  if (lines.length < 2) throw new Error(`${label} is not a valid Tauri Minisign envelope`);
  return lines;
}

function decodeLine(value, expectedLength, label) {
  const decoded = Buffer.from(value, "base64");
  if (decoded.length !== expectedLength) {
    throw new Error(`${label} has an invalid length`);
  }
  return decoded;
}

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tauriBinary = join(
  projectRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const temporaryDirectory = await mkdtemp(join(tmpdir(), "yunqi-updater-keypair-"));
const sentinelPath = join(temporaryDirectory, "sentinel.txt");

try {
  await writeFile(sentinelPath, "YunQi-Watchhouse updater key preflight\n", "utf8");
  const signer = spawnSync(tauriBinary, ["signer", "sign", sentinelPath], {
    cwd: projectRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      TAURI_SIGNING_PRIVATE_KEY: privateKey,
    },
  });
  if (signer.status !== 0) {
    throw new Error("The updater private key or its password could not sign the preflight file");
  }

  const publicLines = decodeEnvelope(publicKey, "Updater public key");
  const signatureEnvelope = await readFile(`${sentinelPath}.sig`, "utf8");
  const signatureLines = decodeEnvelope(signatureEnvelope, "Updater signature");
  if (signatureLines.length < 4 || !signatureLines[2].startsWith("trusted comment: ")) {
    throw new Error("Updater signature has an invalid Minisign structure");
  }

  const publicRecord = decodeLine(publicLines[1], 42, "Updater public key");
  const signatureRecord = decodeLine(signatureLines[1], 74, "Updater signature");
  const globalSignature = decodeLine(signatureLines[3], 64, "Updater global signature");
  const publicAlgorithm = publicRecord.subarray(0, 2).toString("ascii");
  const signatureAlgorithm = signatureRecord.subarray(0, 2).toString("ascii");
  if (!["Ed", "ED"].includes(publicAlgorithm) || signatureAlgorithm !== "ED") {
    throw new Error("Updater key pair uses an unsupported Minisign algorithm");
  }
  if (!publicRecord.subarray(2, 10).equals(signatureRecord.subarray(2, 10))) {
    throw new Error("Updater private and public keys do not belong to the same key pair");
  }

  const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
  const verificationKey = createPublicKey({
    key: Buffer.concat([spkiPrefix, publicRecord.subarray(10)]),
    format: "der",
    type: "spki",
  });
  const sentinel = await readFile(sentinelPath);
  const digest = createHash("blake2b512").update(sentinel).digest();
  const signature = signatureRecord.subarray(10);
  const trustedComment = Buffer.from(signatureLines[2].slice("trusted comment: ".length));
  const signatureValid = verify(null, digest, verificationKey, signature);
  const commentValid = verify(
    null,
    Buffer.concat([signature, trustedComment]),
    verificationKey,
    globalSignature,
  );
  if (!signatureValid || !commentValid) {
    throw new Error("Updater private and public key verification failed");
  }

  console.log("Updater signing key pair verified.");
} finally {
  await rm(temporaryDirectory, { recursive: true, force: true });
}
