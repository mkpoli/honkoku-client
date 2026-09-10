// Prints a fresh Firebase ID token for the saved session, refreshing it through
// the Secure Token API when the stored one has less than five minutes left.
//
//   bun tools/session/token.ts              print the ID token
//   bun tools/session/token.ts --claims     print the decoded token payload instead

import { readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const SESSION = join(homedir(), ".local", "share", "honkoku-client", "session.json");

interface Session {
  apiKey: string;
  uid: string;
  refreshToken: string;
  idToken: string;
  expiresAt: string;
  [k: string]: unknown;
}

export async function idToken(): Promise<string> {
  const session = JSON.parse(readFileSync(SESSION, "utf8")) as Session;
  if (Date.parse(session.expiresAt) - Date.now() > 5 * 60_000) return session.idToken;
  const res = await fetch(`https://securetoken.googleapis.com/v1/token?key=${session.apiKey}`, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ grant_type: "refresh_token", refresh_token: session.refreshToken }),
  });
  if (!res.ok) throw new Error(`token refresh failed: ${res.status} ${await res.text()}`);
  const data = (await res.json()) as { id_token: string; refresh_token: string; expires_in: string };
  session.idToken = data.id_token;
  session.refreshToken = data.refresh_token;
  session.expiresAt = new Date(Date.now() + Number(data.expires_in) * 1000).toISOString();
  writeFileSync(SESSION, JSON.stringify(session, null, 2) + "\n");
  return session.idToken;
}

if (import.meta.main) {
  const token = await idToken();
  if (process.argv.includes("--claims")) {
    const payload = token.split(".")[1];
    console.log(JSON.stringify(JSON.parse(Buffer.from(payload, "base64url").toString("utf8")), null, 2));
  } else {
    console.log(token);
  }
}
