import { writeFile } from "node:fs/promises";

const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY?.trim();
if (!publicKey) {
  throw new Error("TAURI_UPDATER_PUBLIC_KEY is required to create the release config");
}

const output = new URL("../src-tauri/tauri.release.conf.json", import.meta.url);
const config = {
  bundle: {
    targets: ["app", "dmg"],
    createUpdaterArtifacts: true,
  },
  plugins: {
    updater: {
      endpoints: [
        "https://github.com/wangjia5289/YunQi-Watchhouse/releases/latest/download/latest.json",
      ],
      pubkey: publicKey,
    },
  },
};

await writeFile(output, `${JSON.stringify(config, null, 2)}\n`, "utf8");
console.log(`Wrote ${output.pathname}`);
